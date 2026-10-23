# Hackathon Ideen 2026 · Desafio 1 · Motor de regras

Projeto individual do Kauã. Prazo: **segunda 26/10/2026, 9h** (repositório arquivado).
Repositório de entrega: `ideen-tech/hackathon-2026-d1-<usuário>`.

## Fonte de verdade

- `hackathon-2026-d1-kit/ENUNCIADO.md` é a spec. Modelo, 12 regras e semântica estão lá.
- O pptx na raiz é só o briefing (critérios de avaliação e o que entregar).
- Não invente regra nem semântica. Ambiguidade vira "questão em aberto" (seção abaixo) e o Kauã decide.
- O fluxo spec-driven do `~/.claude/CLAUDE.md` vale aqui, mas sem Jira, OpenSpec nem skills de fase:
  o hackathon não é uma task da CERC.

## O problema em uma frase

Classificar cada duplicata de um JSONL pela primeira regra satisfeita (R01..R12, senão `SEM_REGRA`),
com as regras fora do código, acertando os casos de borda e no menor tempo total possível.

## Contrato (fixo)

- Comando: `motor --entrada entrada.jsonl > saida.txt`
- Saída: uma linha por duplicata, `id:<id> - regra: <R01..R12|SEM_REGRA>`. Ordem livre.
- Data de referência: `2026-10-23`.

## Como a banca avalia

| Peso | Critério | O que significa aqui |
|---|---|---|
| 35% | Entregou o mínimo | CLI no contrato, regras fora do código, `Dockerfile`, `evidencias/` dos dois sets |
| 30% | Respeitou o desafio | posição no ranking: 1º acertos no set surpresa, 2º tempo (mediana de 3, 1 CPU, inclui startup) |
| 20% | Reaproveitável | outra pessoa roda sem precisar do Kauã |
| 15% | README | problema, o que foi feito, como reproduzir, evidências, **onde a IA errou** |

Bônus: saída diz quais regras dispararam; segundo domínio sem mudar código; testes automatizados.
**Descartado (Kauã, 23/10):** bônus "quais regras dispararam" fora do escopo. A regex do `medir.py`
exige a linha exata `id:X - regra: Y`; a saída é só essa.
Fora de escopo: servidor HTTP, UI de edição de regras, deploy.

## Princípios do desafio

- **Simples**: poucas peças, entendível em 5 minutos de README.
- **Genérico**: trocar ou reordenar regra não muda o motor.
- **Rápido**: o tempo inclui inicialização do processo (e do container).

## Armadilha principal

Os sets `pequeno` (100) e `grande` (200 mil) do kit são **limpos**: todo campo tem o tipo esperado
(conferido em 23/10). O set surpresa traz campo ausente, tipo errado, valor no limite e regras em
conflito. Acertar 100% no kit **não prova** a semântica. Os casos de borda precisam de testes próprios,
derivados das 8 regras de semântica do enunciado.

Pontos que exigem cuidado na implementação:

- Booleano não é número (em Python, `True` é `int`). `"15000.00"` não é número.
- Campo ausente/nulo/tipo errado torna falsa qualquer comparação, **inclusive `≠`**.
- "Está vazio" é a única condição verdadeira para ausente/nulo/`""`.
- Data só vale `AAAA-MM-DD` e válida (`2026-02-30` é ausente).
- "Começa com" só em texto (`cfop: 5102` numérico → falso).
- Texto compara exato, com maiúsculas e minúsculas e sem trim.

## Decisões de semântica

Regra (Kauã, 23/10): seguir **estritamente** o enunciado, ao pé da letra. Nada de leitura "tolerante".
Cada caso abaixo é aplicação literal das regras 1 a 8 da seção Semântica.

| Caso | Decisão | Base no enunciado |
|---|---|---|
| Inteiro do JSON (`15000`) | é número | tipo "número" = número JSON, inteiro ou decimal |
| Booleano onde se espera número | não é número → comparação falsa | regra 1 (tipo diferente) e regra 4 |
| `NaN`/`Infinity` | não é número JSON → comparação falsa | regra 1 (tipo diferente) |
| Objeto pai ausente ou não objeto (`"financeiro": "x"`) | campos filhos ausentes → comparação falsa | regra 1 (campo ausente) |
| "Está vazio" com tipo errado (`manifestacao.tipo: 0`) | não vazio → falso | regra 2: só ausente, nulo ou `""` |
| Linha que não é JSON válido ou sem `id` | fora do contrato ("cada objeto tem um `id`"); o motor não cai: pula a linha e avisa no stderr | decisão técnica, não muda classificação de nenhum caso válido |

## Decisões técnicas

- **Linguagem: Rust** (Kauã pediu a mais eficiente, consistente e rápida). Binário estático, imagem
  Docker `FROM scratch`, inicialização quase zero, `serde_json` distingue número/booleano/texto.
- **Docker desde o início** (Kauã, 23/10): build, testes e medição rodam em container (`--cpus=1`
  na medição). Rust não está instalado na máquina, e não precisa estar. Nada de "funciona local, quebra no Docker".

- **Leitura enxuta:** só os campos citados pelas regras são lidos de cada linha (`solucao-rust/src/documento.rs`).
  O esquema sai do arquivo de regras: continua genérico. Grande: 0,82 s → 0,45 s.
  No Go + CEL, mesma ideia (`solucao-go_cel/documento.go`, caminhos tirados da AST do CEL),
  com fuzz diferencial contra `json.Unmarshal`. Mais textos sem cópia, mapas reaproveitados por
  linha e `SetGCPercent(800)`: 2,1 s → 0,74 s (23/10). O que resta é ~35% avaliação CEL.

- **Descartados (medidos em 23/10):**
  - Paralelismo: banca usa `--cpus=1` e o motor é ~90% CPU (0,21 s de CPU em 0,23 s). Threads
    dividem a mesma cota e não ganham nada.
  - `sonic-rs` (parser SIMD): 0,45 → 0,41 s no Mac ARM e 100% correto. No x86, porém, o SIMD só
    liga com AVX2 em tempo de compilação: sem a flag, o ganho some; com ela, quebra (SIGILL) numa
    máquina sem AVX2. Risco de zerar os acertos por ~0,04 s. Revertido.
  - Docker (~0,17 s fixos): rede ~0,05, criação do container ~0,07, CLI ~0,04, `--rm` ~0,025.
    Os flags são da banca: não há o que mudar.

## Estrutura e estado (23/10)

```
Dockerfile              oficial (banca): solução Rust
solucao-rust/           motor Rust (regras em árvore JSON com tipo declarado) · 0,45 s no grande
solucao-go_cel/         motor Go + CEL (regras como expressões CEL) · 0,74 s · Dockerfile próprio
testes/                 compartilhado: bordas (gabarito à mão), oráculo Python, gerador de sujos
evidencias/rust|go_cel  saída do medir.py
docs/                   Artifact didático (motor Rust)
hackathon-2026-d1-kit/  kit original (medir.py, dados) — não alterar
```

As duas soluções: 100% em pequeno, grande, 109 bordas e 600 mil sujos × oráculo.
Falta: `git init`, criar o repositório na org e push (confirmar com o Kauã).

Ao mudar regra ou semântica: atualizar `testes/gerar_bordas.py` (esperado escrito à mão a partir
do enunciado) **e** `testes/oraculo.py`; regerar bordas; atualizar as regras **das duas soluções**;
rodar os testes das duas; regerar `evidencias/`. Erro da IA corrigido → registrar em "Onde a IA
errou" no README.

CEL: nunca comparar campo cru. Sempre usar `texto()`/`numero()`/`booleano()`/`data()`, porque o CEL
compara tipos diferentes sem erro (`'SC' != 35` dá verdadeiro), o que viola a regra 1 do enunciado.

**Artifact didático** (https://claude.ai/artifact/JwhG66drsbZmdCZBMxauXr): acompanha o motor Rust.
Regra do Kauã (23/10): toda mudança em regras, semântica, estrutura ou números de evidência atualiza o Artifact.
- Fonte: `docs/motor-passo-a-passo.template.html`. `docs/gerar_artifact.py` embute
  `solucao-rust/regras/duplicatas.json` e gera `docs/motor-passo-a-passo.html`; o JS reimplementa
  a semântica do Rust.
- Mudou semântica → espelhar no JS do template.
- Antes de republicar, rodar `node docs/conferir_artifact.js testes/bordas.jsonl testes/bordas_gabarito.txt`
  (0 divergências) e publicar o mesmo arquivo, de outra sessão com `url`.

## Validação

```bash
cd hackathon-2026-d1-kit
python medir.py --set pequeno --cmd "<comando do motor> --entrada {entrada}"
python medir.py --set grande  --cmd "<comando do motor> --entrada {entrada}"
```

Banca usa Docker com `--cpus=1`. Medir também assim antes de entregar. Saída do `medir.py` vai para
`evidencias/`.

## Regras de trabalho neste repo

- Dados fictícios apenas. Nada de senha, token ou chave em código, prompt ou tela (há varredura de segredos).
- Nenhum código de projeto da CERC entra aqui.
- Anotar no README, à medida que acontece, **onde a IA errou** e como foi corrigido (vale 15%).
- Idioma: pt-BR em código de domínio, README e commits.
