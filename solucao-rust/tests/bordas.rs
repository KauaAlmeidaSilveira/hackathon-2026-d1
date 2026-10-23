//! Roda o motor com as regras reais sobre ../testes/bordas.jsonl (compartilhado entre as soluções)
//! e compara com o gabarito.

use motor::{documento::Lido, regras::Motor};
use std::collections::HashMap;

#[test]
fn casos_de_borda_batem_com_o_gabarito() {
    let raiz = env!("CARGO_MANIFEST_DIR");
    let ler = |p: &str| std::fs::read_to_string(format!("{raiz}/{p}")).unwrap();
    let motor = Motor::carregar(&ler("regras/duplicatas.json")).unwrap();
    let casos: HashMap<String, String> = ler("../testes/bordas_casos.txt")
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(id, resto)| (id.to_string(), resto.to_string())))
        .collect();
    let gabarito: HashMap<String, String> = ler("../testes/bordas_gabarito.txt")
        .lines()
        .filter_map(|l| {
            let (id, regra) = l.strip_prefix("id:")?.split_once(" - regra: ")?;
            Some((id.to_string(), regra.to_string()))
        })
        .collect();

    let mut erros = Vec::new();
    for linha in ler("../testes/bordas.jsonl").lines() {
        let doc = motor.ler_linha(linha.as_bytes()).expect("linha ilegível");
        let Some(Lido::Inteiro(id)) = motor.id(&doc) else { panic!("sem id: {linha}") };
        let id = id.to_string();
        let obtido = motor.classificar(&doc);
        if gabarito[&id] != obtido {
            erros.push(format!("id {id} ({}): obtido {obtido}", casos[&id]));
        }
    }
    assert!(!gabarito.is_empty());
    assert!(erros.is_empty(), "{} erro(s):\n{}", erros.len(), erros.join("\n"));
}
