pub mod documento;
pub mod regras;
pub mod semantica;

/// `NaN`, `Infinity` e `-Infinity` não são JSON válido e fazem o parser estrito recusar a linha.
/// Fora de texto, esses tokens viram `[]`: lista não é texto, número, booleano nem data, e não é
/// "vazio". O valor conta como "tipo diferente do esperado" em qualquer regra, e o id não se perde.
/// Devolve `None` se a linha não tem nenhum desses tokens (o erro era outro).
pub(crate) fn neutralizar_nao_numeros(linha: &[u8]) -> Option<Vec<u8>> {
    const TOKENS: [&[u8]; 3] = [b"-Infinity", b"Infinity", b"NaN"];
    const SUBSTITUTO: &[u8] = b"[]";
    let (mut saida, mut i, mut em_texto, mut trocou) = (Vec::with_capacity(linha.len() + 32), 0, false, false);
    while i < linha.len() {
        let c = linha[i];
        if em_texto {
            saida.push(c);
            if c == b'\\' && i + 1 < linha.len() {
                saida.push(linha[i + 1]);
                i += 1;
            } else if c == b'"' {
                em_texto = false;
            }
        } else if let Some(t) = TOKENS.iter().find(|t| linha[i..].starts_with(t)) {
            saida.extend_from_slice(SUBSTITUTO);
            i += t.len();
            trocou = true;
            continue;
        } else {
            em_texto = c == b'"';
            saida.push(c);
        }
        i += 1;
    }
    trocou.then_some(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nan_e_infinity_viram_lista_fora_de_texto() {
        let linha = br#"{"id":1,"a":NaN,"b":-Infinity,"c":"NaN \"x\" Infinity","d":Infinity}"#;
        let esperado = br#"{"id":1,"a":[],"b":[],"c":"NaN \"x\" Infinity","d":[]}"#;
        assert_eq!(neutralizar_nao_numeros(linha).unwrap(), esperado.to_vec());
    }

    #[test]
    fn linha_sem_nan_nao_e_reescrita() {
        assert!(neutralizar_nao_numeros(br#"{"id":1,"a":}"#).is_none());
    }
}
