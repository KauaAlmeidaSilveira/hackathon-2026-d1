//! motor --entrada entrada.jsonl [--regras regras/duplicatas.json] > saida.txt

use motor::{documento::Lido, regras::Motor};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process::ExitCode;

const REGRAS_PADRAO: &str = "regras/duplicatas.json";

fn main() -> ExitCode {
    match executar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::from(2)
        }
    }
}

fn executar() -> Result<(), String> {
    let (entrada, arquivo_regras) = argumentos()?;
    let texto = std::fs::read_to_string(&arquivo_regras).map_err(|e| format!("{arquivo_regras}: {e}"))?;
    let motor = Motor::carregar(&texto).map_err(|e| format!("{arquivo_regras}: {e}"))?;

    let arquivo = File::open(&entrada).map_err(|e| format!("{entrada}: {e}"))?;
    let mut leitor = BufReader::with_capacity(1 << 20, arquivo);
    let mut saida = BufWriter::with_capacity(1 << 20, io::stdout().lock());
    let mut linha = Vec::with_capacity(4096);
    let mut numero_linha = 0u64;

    loop {
        linha.clear();
        if leitor.read_until(b'\n', &mut linha).map_err(|e| format!("{entrada}: {e}"))? == 0 {
            break;
        }
        numero_linha += 1;
        // BOM do UTF-8 no início do arquivo não faz parte do JSON.
        let conteudo = linha.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&linha).trim_ascii();
        if conteudo.is_empty() {
            continue;
        }
        // Fora do contrato (JSON inválido ou sem id): avisa no stderr e segue.
        let Some(doc) = motor.ler_linha(conteudo) else {
            eprintln!("aviso: linha {numero_linha}: JSON inválido, ignorada");
            continue;
        };
        let codigo = motor.classificar(&doc);
        let escrita = match motor.id(&doc) {
            Some(Lido::Inteiro(i)) => write!(saida, "id:{i}"),
            Some(Lido::Numero(n)) => write!(saida, "id:{}", serde_json::Value::from(*n)),
            Some(Lido::Texto(s)) => write!(saida, "id:{s}"),
            _ => {
                eprintln!("aviso: linha {numero_linha}: sem id, ignorada");
                continue;
            }
        };
        escrita
            .and_then(|_| saida.write_all(b" - regra: "))
            .and_then(|_| saida.write_all(codigo.as_bytes()))
            .and_then(|_| saida.write_all(b"\n"))
            .map_err(|e| format!("stdout: {e}"))?;
    }
    saida.flush().map_err(|e| format!("stdout: {e}"))
}

fn argumentos() -> Result<(String, String), String> {
    let uso = "uso: motor --entrada <arquivo.jsonl> [--regras <regras.json>]";
    let (mut entrada, mut regras) = (None, REGRAS_PADRAO.to_string());
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--entrada" => entrada = Some(args.next().ok_or(uso)?),
            "--regras" => regras = args.next().ok_or(uso)?,
            _ => return Err(uso.into()),
        }
    }
    Ok((entrada.ok_or(uso)?, regras))
}
