//! Leitura de uma linha do JSONL guardando só os campos que as regras citam.
//!
//! O esquema (árvore de caminhos → posição) sai do arquivo de regras, não do código. Cada linha
//! vira um vetor de posições; o resto do JSON é validado e descartado sem alocar. Para o motor,
//! o resultado é o mesmo de ler o JSON inteiro: campo ausente = `None`; chave repetida = vale a
//! última; pai que não é objeto = filhos ausentes.

use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::borrow::Cow;
use std::fmt;

/// Valor de um campo citado pelas regras, sem conversão de tipo.
#[derive(Clone, Debug, PartialEq)]
pub enum Lido<'de> {
    Nulo,
    Texto(Cow<'de, str>),
    Inteiro(i128),
    Numero(f64),
    Booleano(bool),
    /// Lista ou objeto: nunca é texto, número, booleano, data nem "vazio".
    Outro,
}

pub type Documento<'de> = Vec<Option<Lido<'de>>>;

#[derive(Default)]
pub struct Esquema {
    raiz: No,
    total: usize,
}

#[derive(Default)]
struct No {
    filhos: Vec<(String, No)>,
    posicao: Option<usize>,
    /// Posições do próprio nó e de todos abaixo dele: limpas quando a chave aparece de novo.
    abaixo: Vec<usize>,
}

impl Esquema {
    /// Registra o caminho (`logistica.migo`) e devolve a posição dele no documento.
    pub fn posicao(&mut self, caminho: &str) -> usize {
        let (total, mut no) = (&mut self.total, &mut self.raiz);
        for parte in caminho.split('.') {
            let i = match no.filhos.iter().position(|(k, _)| k == parte) {
                Some(i) => i,
                None => {
                    no.filhos.push((parte.to_string(), No::default()));
                    no.filhos.len() - 1
                }
            };
            no = &mut no.filhos[i].1;
        }
        *no.posicao.get_or_insert_with(|| {
            *total += 1;
            *total - 1
        })
    }

    /// Chamar depois de registrar todos os caminhos.
    pub fn finalizar(&mut self) {
        fn preencher(no: &mut No) -> Vec<usize> {
            let mut todas: Vec<usize> = no.posicao.into_iter().collect();
            for (_, filho) in &mut no.filhos {
                todas.extend(preencher(filho));
            }
            no.abaixo = todas.clone();
            todas
        }
        preencher(&mut self.raiz);
    }

    /// `None` se a linha não for um objeto JSON válido.
    pub fn ler<'de>(&self, linha: &'de [u8]) -> Option<Documento<'de>> {
        let mut doc = vec![None; self.total];
        let mut de = serde_json::Deserializer::from_slice(linha);
        de.deserialize_map(ValorVisitor { no: &self.raiz, doc: &mut doc }).ok()?;
        de.end().ok()?;
        Some(doc)
    }
}

/// Mesma leitura, mas sem emprestar da linha (usada quando a linha precisou ser reescrita).
pub fn possuir(doc: Documento<'_>) -> Documento<'static> {
    doc.into_iter()
        .map(|v| {
            v.map(|l| match l {
                Lido::Texto(s) => Lido::Texto(Cow::Owned(s.into_owned())),
                Lido::Nulo => Lido::Nulo,
                Lido::Inteiro(i) => Lido::Inteiro(i),
                Lido::Numero(n) => Lido::Numero(n),
                Lido::Booleano(b) => Lido::Booleano(b),
                Lido::Outro => Lido::Outro,
            })
        })
        .collect()
}

struct Chave<'a>(&'a No);

impl<'de> DeserializeSeed<'de> for Chave<'_> {
    type Value = Option<usize>;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for Chave<'_> {
    type Value = Option<usize>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("chave")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(self.0.filhos.iter().position(|(k, _)| k == v))
    }
}

struct ValorVisitor<'a, 'de> {
    no: &'a No,
    doc: &'a mut Documento<'de>,
}

impl<'de> ValorVisitor<'_, 'de> {
    fn gravar<E>(self, v: Lido<'de>) -> Result<(), E> {
        if let Some(p) = self.no.posicao {
            self.doc[p] = Some(v);
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for ValorVisitor<'_, 'de> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for ValorVisitor<'_, 'de> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("valor JSON")
    }
    fn visit_bool<E>(self, v: bool) -> Result<(), E> {
        self.gravar(Lido::Booleano(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<(), E> {
        self.gravar(Lido::Inteiro(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<(), E> {
        self.gravar(Lido::Inteiro(v.into()))
    }
    fn visit_f64<E>(self, v: f64) -> Result<(), E> {
        self.gravar(Lido::Numero(v))
    }
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<(), E> {
        self.gravar(Lido::Texto(Cow::Borrowed(v)))
    }
    fn visit_str<E>(self, v: &str) -> Result<(), E> {
        self.gravar(Lido::Texto(Cow::Owned(v.to_string())))
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        self.gravar(Lido::Nulo)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        self.gravar(Lido::Outro)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(chave) = map.next_key_seed(Chave(self.no))? {
            match chave {
                Some(i) => {
                    let filho = &self.no.filhos[i].1;
                    for &p in &filho.abaixo {
                        self.doc[p] = None;
                    }
                    map.next_value_seed(ValorVisitor { no: filho, doc: self.doc })?;
                }
                None => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        self.gravar(Lido::Outro)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn esquema(caminhos: &[&str]) -> (Esquema, Vec<usize>) {
        let mut e = Esquema::default();
        let p = caminhos.iter().map(|c| e.posicao(c)).collect();
        e.finalizar();
        (e, p)
    }

    #[test]
    fn le_so_os_campos_citados() {
        let (e, p) = esquema(&["a", "b.c", "b.d"]);
        let doc = e.ler(br#"{"a":"x","z":[1,{"q":2}],"b":{"c":1.5,"d":null,"e":"y"}}"#).unwrap();
        assert_eq!(doc[p[0]], Some(Lido::Texto("x".into())));
        assert_eq!(doc[p[1]], Some(Lido::Numero(1.5)));
        assert_eq!(doc[p[2]], Some(Lido::Nulo));
    }

    #[test]
    fn pai_que_nao_e_objeto_deixa_filhos_ausentes() {
        let (e, p) = esquema(&["b.c"]);
        assert_eq!(e.ler(br#"{"b":"x"}"#).unwrap()[p[0]], None);
        assert_eq!(e.ler(br#"{"b":[1]}"#).unwrap()[p[0]], None);
        assert_eq!(e.ler(br#"{}"#).unwrap()[p[0]], None);
    }

    #[test]
    fn chave_repetida_vale_a_ultima() {
        let (e, p) = esquema(&["a", "b.c"]);
        let doc = e.ler(br#"{"a":1,"a":"x","b":{"c":1},"b":{"d":2}}"#).unwrap();
        assert_eq!(doc[p[0]], Some(Lido::Texto("x".into())));
        assert_eq!(doc[p[1]], None);
    }

    #[test]
    fn texto_com_escape_e_chave_com_escape() {
        let (e, p) = esquema(&["status"]);
        let doc = e.ler(br#"{"status":"ATIVA"}"#).unwrap();
        assert_eq!(doc[p[0]], Some(Lido::Texto("ATIVA".into())));
    }

    #[test]
    fn linha_invalida() {
        let (e, _) = esquema(&["a"]);
        assert!(e.ler(br#"{"a":}"#).is_none());
        assert!(e.ler(br#"[1,2]"#).is_none());
        assert!(e.ler(br#"{"a":1} lixo"#).is_none());
    }
}
