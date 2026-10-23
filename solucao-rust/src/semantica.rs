//! Leitura tipada de campos, conforme a seção "Semântica" do ENUNCIADO.
//!
//! Toda leitura devolve `None` quando o campo está ausente, nulo ou com tipo diferente do
//! declarado. Quem compara trata `None` como condição falsa (regra 1), inclusive para `!=`.

use crate::documento::Lido;

/// Tipo que a regra espera para o campo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tipo {
    Texto,
    Numero,
    Booleano,
    Data,
}

/// Valor lido do documento (ou literal da regra), já no tipo esperado.
/// Data vira inteiro AAAAMMDD, o que preserva a ordem cronológica.
#[derive(Clone, Debug, PartialEq)]
pub enum Escalar<'a> {
    Texto(&'a str),
    Numero(f64),
    Booleano(bool),
    Data(u32),
}

/// Lê o valor no tipo esperado. Qualquer divergência de tipo devolve `None`.
/// - Número: só número JSON (booleano não é número; `"15000.00"` não é número).
/// - Booleano: só `true`/`false` do JSON (regra 4).
/// - Data: só texto `AAAA-MM-DD` com data válida no calendário (regra 5).
pub fn ler<'a>(v: &'a Lido, tipo: Tipo) -> Option<Escalar<'a>> {
    match (tipo, v) {
        (Tipo::Texto, Lido::Texto(s)) => Some(Escalar::Texto(s)),
        (Tipo::Numero, Lido::Inteiro(i)) => Some(Escalar::Numero(*i as f64)),
        (Tipo::Numero, Lido::Numero(n)) => Some(Escalar::Numero(*n)),
        (Tipo::Booleano, Lido::Booleano(b)) => Some(Escalar::Booleano(*b)),
        (Tipo::Data, Lido::Texto(s)) => data(s).map(Escalar::Data),
        _ => None,
    }
}

/// "Está vazio": ausente, nulo ou texto vazio (regra 2). Qualquer outro valor não é vazio.
pub fn vazio(v: Option<&Lido>) -> bool {
    match v {
        None | Some(Lido::Nulo) => true,
        Some(Lido::Texto(s)) => s.is_empty(),
        Some(_) => false,
    }
}

/// Converte `AAAA-MM-DD` em AAAAMMDD. Formato diferente ou data inexistente: `None`.
pub fn data(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |faixa: &[u8]| -> Option<u32> {
        faixa.iter().try_fold(0u32, |acc, &c| {
            c.is_ascii_digit().then(|| acc * 10 + u32::from(c - b'0'))
        })
    };
    let (ano, mes, dia) = (num(&b[0..4])?, num(&b[5..7])?, num(&b[8..10])?);
    let bissexto = (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0;
    let dias_no_mes = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bissexto => 29,
        2 => 28,
        _ => return None,
    };
    // Não existe ano 0 no calendário gregoriano.
    if ano == 0 || dia == 0 || dia > dias_no_mes {
        return None;
    }
    Some(ano * 10000 + mes * 100 + dia)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn t(s: &str) -> Lido<'_> {
        Lido::Texto(s.into())
    }

    #[test]
    fn numero_so_aceita_numero_json() {
        assert_eq!(ler(&Lido::Inteiro(15000), Tipo::Numero), Some(Escalar::Numero(15000.0)));
        assert_eq!(ler(&Lido::Numero(15000.5), Tipo::Numero), Some(Escalar::Numero(15000.5)));
        assert_eq!(ler(&t("15000.00"), Tipo::Numero), None);
        assert_eq!(ler(&Lido::Booleano(true), Tipo::Numero), None);
        assert_eq!(ler(&Lido::Nulo, Tipo::Numero), None);
        assert_eq!(ler(&Lido::Outro, Tipo::Numero), None);
    }

    #[test]
    fn booleano_so_aceita_true_false() {
        assert_eq!(ler(&Lido::Booleano(false), Tipo::Booleano), Some(Escalar::Booleano(false)));
        assert_eq!(ler(&t("false"), Tipo::Booleano), None);
        assert_eq!(ler(&Lido::Inteiro(0), Tipo::Booleano), None);
        assert_eq!(ler(&Lido::Inteiro(1), Tipo::Booleano), None);
    }

    #[test]
    fn texto_nao_aceita_numero() {
        assert_eq!(ler(&Lido::Inteiro(5102), Tipo::Texto), None);
        assert_eq!(ler(&t("5102"), Tipo::Texto), Some(Escalar::Texto("5102")));
    }

    #[test]
    fn data_valida_e_invalida() {
        assert_eq!(data("2026-10-23"), Some(20261023));
        assert_eq!(data("2024-02-29"), Some(20240229));
        assert_eq!(data("2026-02-29"), None);
        assert_eq!(data("2026-02-30"), None);
        assert_eq!(data("2026-13-01"), None);
        assert_eq!(data("2026-00-10"), None);
        assert_eq!(data("2026-04-31"), None);
        assert_eq!(data("1900-02-29"), None);
        assert_eq!(data("2000-02-29"), Some(20000229));
        assert_eq!(data("0000-01-01"), None);
        assert_eq!(data("2026-1-01"), None);
        assert_eq!(data("2026/10/23"), None);
        assert_eq!(data("23-10-2026"), None);
        assert_eq!(data("2026-10-23T00:00:00"), None);
        assert_eq!(data(" 2026-10-2"), None);
        assert_eq!(data(""), None);
        assert_eq!(ler(&Lido::Inteiro(20261023), Tipo::Data), None);
    }

    #[test]
    fn vazio_so_para_ausente_nulo_ou_texto_vazio() {
        assert!(vazio(None));
        assert!(vazio(Some(&Lido::Nulo)));
        assert!(vazio(Some(&t(""))));
        assert!(!vazio(Some(&t(" "))));
        assert!(!vazio(Some(&Lido::Inteiro(0))));
        assert!(!vazio(Some(&Lido::Booleano(false))));
        assert!(!vazio(Some(&Lido::Outro)));
        assert!(!vazio(Some(&t("ACTO"))));
    }
}
