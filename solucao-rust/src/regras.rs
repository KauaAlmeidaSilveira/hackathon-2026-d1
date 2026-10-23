//! Carrega o arquivo de regras (JSON), valida e compila em uma árvore de condições.
//! O motor não conhece nenhuma regra de negócio: tudo vem do arquivo.

use crate::documento::{possuir, Documento, Esquema, Lido};
use crate::semantica::{ler, vazio, Escalar, Tipo};
use serde_json::{Map, Value};

pub struct Motor {
    regras: Vec<Regra>,
    sem_regra: String,
    /// Só os campos citados pelas regras (mais o `id`) são lidos de cada linha.
    esquema: Esquema,
    posicao_id: usize,
}

struct Regra {
    codigo: String,
    condicao: Condicao,
}

enum Condicao {
    Todas(Vec<Condicao>),
    Alguma(Vec<Condicao>),
    Vazio(usize),
    Comparar { campo: usize, tipo: Tipo, op: Op, alvo: Alvo },
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Igual,
    Diferente,
    Menor,
    MenorIgual,
    Maior,
    MaiorIgual,
    Em,
    ComecaCom,
}

enum Alvo {
    Literal(Literal),
    Lista(Vec<Literal>),
    Campo(usize),
}

enum Literal {
    Texto(String),
    Numero(f64),
    Booleano(bool),
    Data(u32),
}

impl Literal {
    fn escalar(&self) -> Escalar<'_> {
        match self {
            Literal::Texto(s) => Escalar::Texto(s),
            Literal::Numero(n) => Escalar::Numero(*n),
            Literal::Booleano(b) => Escalar::Booleano(*b),
            Literal::Data(d) => Escalar::Data(*d),
        }
    }
}

impl Motor {
    /// Devolve o código da primeira regra satisfeita, ou o código de "sem regra".
    pub fn classificar(&self, doc: &Documento) -> &str {
        self.regras
            .iter()
            .find(|r| r.condicao.avaliar(doc))
            .map_or(&self.sem_regra, |r| &r.codigo)
    }

    /// Lê uma linha do JSONL. Se o parser estrito recusar a linha só por causa de `NaN`/`Infinity`,
    /// a linha é reescrita (ver `neutralizar_nao_numeros`) e lida de novo.
    pub fn ler_linha<'de>(&self, linha: &'de [u8]) -> Option<Documento<'de>> {
        self.esquema
            .ler(linha)
            .or_else(|| Some(possuir(self.esquema.ler(&crate::neutralizar_nao_numeros(linha)?)?)))
    }

    pub fn id<'a>(&self, doc: &'a Documento) -> Option<&'a Lido<'a>> {
        doc[self.posicao_id].as_ref()
    }

    pub fn carregar(texto: &str) -> Result<Motor, String> {
        let raiz: Value = serde_json::from_str(texto).map_err(|e| format!("JSON inválido: {e}"))?;
        let raiz = objeto(&raiz, "raiz", &["dominio", "descricao", "variaveis", "sem_regra", "regras"])?;
        let vazio_map = Map::new();
        let variaveis = match raiz.get("variaveis") {
            None => &vazio_map,
            Some(v) => v.as_object().ok_or("`variaveis` precisa ser objeto")?,
        };
        let sem_regra = texto_obrigatorio(raiz, "sem_regra", "raiz")?.to_string();
        let lista = raiz.get("regras").and_then(Value::as_array).ok_or("`regras` precisa ser lista")?;
        let mut esquema = Esquema::default();
        let posicao_id = esquema.posicao("id");

        let mut regras = Vec::with_capacity(lista.len());
        for (i, r) in lista.iter().enumerate() {
            let onde = format!("regra #{}", i + 1);
            let r = objeto(r, &onde, &["codigo", "categoria", "descricao", "condicao"])?;
            let codigo = texto_obrigatorio(r, "codigo", &onde)?.to_string();
            if regras.iter().any(|x: &Regra| x.codigo == codigo) || codigo == sem_regra {
                return Err(format!("código repetido: {codigo}"));
            }
            let no = r.get("condicao").ok_or(format!("{codigo}: falta `condicao`"))?;
            let condicao = compilar(no, variaveis, &mut esquema).map_err(|e| format!("{codigo}: {e}"))?;
            regras.push(Regra { codigo, condicao });
        }
        esquema.finalizar();
        Ok(Motor { regras, sem_regra, esquema, posicao_id })
    }
}

impl Condicao {
    fn avaliar(&self, doc: &Documento) -> bool {
        match self {
            Condicao::Todas(cs) => cs.iter().all(|c| c.avaliar(doc)),
            Condicao::Alguma(cs) => cs.iter().any(|c| c.avaliar(doc)),
            Condicao::Vazio(campo) => vazio(doc[*campo].as_ref()),
            Condicao::Comparar { campo, tipo, op, alvo } => {
                // Campo ausente, nulo ou de outro tipo: condição falsa, inclusive para `!=`.
                let Some(a) = doc[*campo].as_ref().and_then(|v| ler(v, *tipo)) else {
                    return false;
                };
                match alvo {
                    Alvo::Literal(l) => comparar(*op, &a, &l.escalar()),
                    Alvo::Lista(ls) => ls.iter().any(|l| a == l.escalar()),
                    Alvo::Campo(outro) => match doc[*outro].as_ref().and_then(|v| ler(v, *tipo)) {
                        Some(b) => comparar(*op, &a, &b),
                        None => false,
                    },
                }
            }
        }
    }
}

fn comparar(op: Op, a: &Escalar, b: &Escalar) -> bool {
    use std::cmp::Ordering::*;
    let ordem = match (a, b) {
        (Escalar::Numero(x), Escalar::Numero(y)) => x.partial_cmp(y),
        (Escalar::Data(x), Escalar::Data(y)) => Some(x.cmp(y)),
        _ => None,
    };
    match op {
        Op::Igual => a == b,
        Op::Diferente => a != b,
        Op::Menor => ordem == Some(Less),
        Op::MenorIgual => matches!(ordem, Some(Less | Equal)),
        Op::Maior => ordem == Some(Greater),
        Op::MaiorIgual => matches!(ordem, Some(Greater | Equal)),
        Op::ComecaCom => matches!((a, b), (Escalar::Texto(x), Escalar::Texto(y)) if x.starts_with(y)),
        Op::Em => unreachable!("`em` é avaliado como lista"),
    }
}

fn compilar(no: &Value, variaveis: &Map<String, Value>, esquema: &mut Esquema) -> Result<Condicao, String> {
    let obj = no.as_object().ok_or("condição precisa ser objeto")?;
    for (chave, construtor) in [("todas", Condicao::Todas as fn(_) -> _), ("alguma", Condicao::Alguma)] {
        if let Some(filhos) = obj.get(chave) {
            objeto(no, chave, &[chave])?;
            let filhos = filhos.as_array().filter(|a| !a.is_empty());
            let filhos = filhos.ok_or(format!("`{chave}` precisa ser lista não vazia"))?;
            let compilados = filhos.iter().map(|f| compilar(f, variaveis, esquema)).collect::<Result<_, _>>()?;
            return Ok(construtor(compilados));
        }
    }

    let campo = esquema.posicao(texto_obrigatorio(obj, "campo", "condição")?);
    let op = texto_obrigatorio(obj, "op", "condição")?;
    if op == "vazio" {
        objeto(no, "condição `vazio`", &["campo", "op"])?;
        return Ok(Condicao::Vazio(campo));
    }
    objeto(no, "condição", &["campo", "tipo", "op", "valor", "variavel", "campo_comparado"])?;

    let tipo = match texto_obrigatorio(obj, "tipo", "condição")? {
        "texto" => Tipo::Texto,
        "numero" => Tipo::Numero,
        "booleano" => Tipo::Booleano,
        "data" => Tipo::Data,
        t => return Err(format!("tipo desconhecido: {t}")),
    };
    let op = match op {
        "=" => Op::Igual,
        "!=" => Op::Diferente,
        "<" => Op::Menor,
        "<=" => Op::MenorIgual,
        ">" => Op::Maior,
        ">=" => Op::MaiorIgual,
        "em" => Op::Em,
        "comeca_com" => Op::ComecaCom,
        o => return Err(format!("operador desconhecido: {o}")),
    };
    let ordenavel = matches!(tipo, Tipo::Numero | Tipo::Data);
    if matches!(op, Op::Menor | Op::MenorIgual | Op::Maior | Op::MaiorIgual) && !ordenavel {
        return Err("`<`, `<=`, `>`, `>=` só valem para numero ou data".into());
    }
    if op == Op::ComecaCom && tipo != Tipo::Texto {
        return Err("`comeca_com` só vale para texto".into());
    }

    let alvos = ["valor", "variavel", "campo_comparado"].map(|k| obj.get(k));
    let alvo = match alvos {
        [Some(v), None, None] => literal_ou_lista(v, tipo, op)?,
        [None, Some(nome), None] => {
            let nome = nome.as_str().ok_or("`variavel` precisa ser texto")?;
            let v = variaveis.get(nome).ok_or(format!("variável não definida: {nome}"))?;
            literal_ou_lista(v, tipo, op)?
        }
        [None, None, Some(outro)] if op != Op::Em => {
            Alvo::Campo(esquema.posicao(outro.as_str().ok_or("`campo_comparado` precisa ser texto")?))
        }
        _ => return Err("informe exatamente um de `valor`, `variavel` ou `campo_comparado`".into()),
    };
    Ok(Condicao::Comparar { campo, tipo, op, alvo })
}

fn literal_ou_lista(v: &Value, tipo: Tipo, op: Op) -> Result<Alvo, String> {
    if op == Op::Em {
        let lista = v.as_array().filter(|a| !a.is_empty()).ok_or("`em` precisa de lista não vazia")?;
        return Ok(Alvo::Lista(lista.iter().map(|x| literal(x, tipo)).collect::<Result<_, _>>()?));
    }
    literal(v, tipo).map(Alvo::Literal)
}

/// O literal da regra precisa estar no tipo declarado; erro de digitação nas regras falha na carga.
fn literal(v: &Value, tipo: Tipo) -> Result<Literal, String> {
    let bruto = match v {
        Value::String(s) => Lido::Texto(s.as_str().into()),
        Value::Number(n) => n.as_f64().map_or(Lido::Outro, Lido::Numero),
        Value::Bool(b) => Lido::Booleano(*b),
        _ => Lido::Outro,
    };
    let lido = ler(&bruto, tipo).ok_or(format!("valor {v} não é do tipo {tipo:?}"))?;
    Ok(match lido {
        Escalar::Texto(s) => Literal::Texto(s.to_string()),
        Escalar::Numero(n) => Literal::Numero(n),
        Escalar::Booleano(b) => Literal::Booleano(b),
        Escalar::Data(d) => Literal::Data(d),
    })
}

fn objeto<'a>(v: &'a Value, onde: &str, permitidas: &[&str]) -> Result<&'a Map<String, Value>, String> {
    let obj = v.as_object().ok_or(format!("{onde}: precisa ser objeto"))?;
    match obj.keys().find(|k| !permitidas.contains(&k.as_str())) {
        Some(k) => Err(format!("{onde}: chave desconhecida `{k}`")),
        None => Ok(obj),
    }
}

fn texto_obrigatorio<'a>(obj: &'a Map<String, Value>, chave: &str, onde: &str) -> Result<&'a str, String> {
    obj.get(chave).and_then(Value::as_str).ok_or(format!("{onde}: `{chave}` precisa ser texto"))
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    impl Motor {
        fn classificar_json(&self, v: &Value) -> String {
            self.classificar(&self.ler_linha(v.to_string().as_bytes()).unwrap()).to_string()
        }
    }

    fn motor(condicao: Value) -> Motor {
        let regras = json!({"sem_regra": "NADA", "variaveis": {"hoje": "2026-10-23"},
                            "regras": [{"codigo": "X", "condicao": condicao}]});
        Motor::carregar(&regras.to_string()).unwrap()
    }

    #[test]
    fn diferente_com_campo_ausente_e_falso() {
        let m = motor(json!({"campo": "a", "tipo": "texto", "op": "!=", "campo_comparado": "b"}));
        assert_eq!(m.classificar_json(&json!({"a": "SP", "b": "RJ"})), "X");
        assert_eq!(m.classificar_json(&json!({"a": "SP", "b": "SP"})), "NADA");
        assert_eq!(m.classificar_json(&json!({"a": "SP"})), "NADA");
        assert_eq!(m.classificar_json(&json!({"a": "SP", "b": null})), "NADA");
        assert_eq!(m.classificar_json(&json!({"a": "SP", "b": 35})), "NADA");
    }

    #[test]
    fn limites_inclusivos_e_exclusivos() {
        let ge = motor(json!({"campo": "v", "tipo": "numero", "op": ">=", "valor": 100000}));
        assert_eq!(ge.classificar_json(&json!({"v": 100000})), "X");
        assert_eq!(ge.classificar_json(&json!({"v": 100000.0})), "X");
        assert_eq!(ge.classificar_json(&json!({"v": 99999.99})), "NADA");
        assert_eq!(ge.classificar_json(&json!({"v": "100000"})), "NADA");
        let gt = motor(json!({"campo": "v", "tipo": "numero", "op": ">", "valor": 500}));
        assert_eq!(gt.classificar_json(&json!({"v": 500})), "NADA");
        assert_eq!(gt.classificar_json(&json!({"v": 500.01})), "X");
    }

    #[test]
    fn data_contra_variavel() {
        let m = motor(json!({"campo": "d", "tipo": "data", "op": "<", "variavel": "hoje"}));
        assert_eq!(m.classificar_json(&json!({"d": "2026-10-22"})), "X");
        assert_eq!(m.classificar_json(&json!({"d": "2026-10-23"})), "NADA");
        assert_eq!(m.classificar_json(&json!({"d": "2026-02-30"})), "NADA");
        assert_eq!(m.classificar_json(&json!({"d": "22/10/2026"})), "NADA");
    }

    #[test]
    fn comeca_com_so_em_texto() {
        let m = motor(json!({"campo": "c", "tipo": "texto", "op": "comeca_com", "valor": "5"}));
        assert_eq!(m.classificar_json(&json!({"c": "5102"})), "X");
        assert_eq!(m.classificar_json(&json!({"c": 5102})), "NADA");
        assert_eq!(m.classificar_json(&json!({"c": "6102"})), "NADA");
    }

    #[test]
    fn lista_e_texto_exato() {
        let m = motor(json!({"campo": "s", "tipo": "texto", "op": "em", "valor": ["CANCELADA", "LIQUIDADA"]}));
        assert_eq!(m.classificar_json(&json!({"s": "LIQUIDADA"})), "X");
        assert_eq!(m.classificar_json(&json!({"s": "liquidada"})), "NADA");
        assert_eq!(m.classificar_json(&json!({"s": "LIQUIDADA "})), "NADA");
    }

    #[test]
    fn regras_invalidas_falham_na_carga() {
        let carregar = |c: Value| Motor::carregar(&json!({"sem_regra": "N", "regras": [{"codigo": "X", "condicao": c}]}).to_string());
        assert!(carregar(json!({"campo": "a", "tipo": "texto", "op": ">", "valor": "x"})).is_err());
        assert!(carregar(json!({"campo": "a", "tipo": "numero", "op": "=", "valor": "10"})).is_err());
        assert!(carregar(json!({"campo": "a", "tipo": "texto", "op": "=", "valor": "x", "extra": 1})).is_err());
        assert!(carregar(json!({"campo": "a", "tipo": "texto", "op": "=", "variavel": "nao_existe"})).is_err());
        assert!(carregar(json!({"todas": []})).is_err());
    }
}
