# Motor de regras de duplicatas escriturais

Hackathon Ideen 2026 · Desafio 1 · São Paulo

## O problema

Classificar cada duplicata de um arquivo JSONL pela primeira regra que ela satisfaz (R01 a R12,
senão `SEM_REGRA`). As regras ficam fora do código, a semântica de dado sujo é estrita, e o
comando precisa terminar o mais rápido possível.

## Duas soluções, mesmos resultados

| | [solucao-rust](solucao-rust/) | [solucao-go_cel](solucao-go_cel/) |
|---|---|---|
| Como as regras são escritas | árvore de condições em JSON, com o tipo declarado em cada campo | expressões [CEL](https://cel.dev) (Common Expression Language, do Google) |
| Acertos (kit, 109 bordas, 600 mil sujos) | 100% | 100% |
| Set grande, Docker com 1 CPU | **0,45 s** | 0,74 s |
| Imagem Docker | ~1 MB | ~10 MB |
| Ponto forte | velocidade | linguagem de regras padrão de mercado (Kubernetes, Envoy, Firebase) |

**O `Dockerfile` da raiz, que é o que a banca roda, usa a solução Rust.** A solução Go + CEL fica
como alternativa, para quem prefere escrever regras numa linguagem conhecida e aceita pagar em
tempo por isso.

A mesma regra nas duas soluções:

```jsonc
// solucao-rust/regras/duplicatas.json
{ "codigo": "R09", "condicao": { "todas": [
    { "campo": "logistica.modal",   "tipo": "texto",  "op": "=", "valor": "AEREO" },
    { "campo": "logistica.peso_kg", "tipo": "numero", "op": ">", "valor": 500 } ] } }

// solucao-go_cel/regras/duplicatas.json
{ "codigo": "R09", "condicao": "texto(logistica.modal) == 'AEREO' && numero(logistica.peso_kg) > 500" }
```

Nas duas, trocar, reordenar ou criar regras é só editar o arquivo de regras. O motor não muda.

## Semântica do enunciado (vale para as duas)

Todas as decisões seguem o enunciado ao pé da letra.

| Caso | Resultado |
|---|---|
| Inteiro do JSON (`15000`) | é número |
| Booleano onde se espera número; `"false"`, `0` ou `1` onde se espera booleano | tipo errado, condição falsa |
| `NaN`, `Infinity` (não são JSON válido) | tipo errado, condição falsa; a linha não se perde |
| Pai ausente ou que não é objeto (`"financeiro": "x"`) | campos filhos ausentes |
| Campo ausente, nulo ou de outro tipo em qualquer comparação, inclusive `!=` | condição falsa |
| "Está vazio" | só ausente, nulo ou `""`; `" "`, `0` e `false` não são vazios |
| Data | só `AAAA-MM-DD` existente no calendário (`2026-02-30`, `2025-02-29` e `2026-9-30` não valem) |
| Texto | igualdade exata: sem ignorar caixa e sem `trim` |
| Linha que não é JSON ou sem `id` (fora do contrato) | aviso no stderr; o motor segue |

## Solução Rust

| Peça | Onde | O que faz |
|---|---|---|
| Regras | [regras/duplicatas.json](solucao-rust/regras/duplicatas.json) | as 12 regras, a data de referência e o código de "sem regra" |
| Motor | [src/regras.rs](solucao-rust/src/regras.rs) | valida o arquivo de regras, compila numa árvore de condições, avalia em ordem |
| Semântica | [src/semantica.rs](solucao-rust/src/semantica.rs) | leitura tipada: número, texto, booleano, data válida, "está vazio" |
| Leitura | [src/documento.rs](solucao-rust/src/documento.rs) | lê de cada linha só os campos que as regras citam |
| CLI | [src/main.rs](solucao-rust/src/main.rs) | `motor --entrada <arquivo> [--regras <arquivo>]` |

- Conectores `todas` (E) e `alguma` (OU), aninháveis. Operadores `=`, `!=`, `<`, `<=`, `>`, `>=`,
  `em`, `comeca_com` e `vazio`. O alvo da comparação pode ser `valor`, `variavel` ou `campo_comparado`.
- Um arquivo de regras inválido (operador desconhecido, literal com tipo errado, chave com erro de
  digitação, `>` em texto) falha na carga com mensagem clara.
- **Desempenho:** binário estático e imagem `FROM scratch`. O motor lê só os campos citados pelas
  regras, e esse conjunto sai do próprio arquivo de regras, então o motor continua genérico. Isso
  levou o set grande de 0,82 s para 0,45 s.

Uma página interativa mostra as seis etapas do motor Rust sobre exemplos editáveis:
[docs/motor-passo-a-passo.html](docs/motor-passo-a-passo.html).

## Solução Go + CEL

| Peça | Onde | O que faz |
|---|---|---|
| Regras | [regras/duplicatas.json](solucao-go_cel/regras/duplicatas.json) | as 12 regras como expressões CEL |
| Motor | [motor.go](solucao-go_cel/motor.go) | compila e checa cada expressão na carga; avalia em ordem |
| Leitura | [documento.go](solucao-go_cel/documento.go) | lê de cada linha só os campos que as expressões citam |
| CLI | [main.go](solucao-go_cel/main.go) | mesmo contrato da solução Rust |

CEL, por padrão, não segue a semântica do enunciado: compara texto com número sem erro, e
`'SC' != 35` dá verdadeiro. Por isso o motor expõe funções que exigem o tipo:
`texto()`, `numero()`, `booleano()`, `data()` e `vazio()`. Campo ausente ou de outro tipo vira erro
de avaliação, e regra com erro conta como falsa. Com `&&` e `||`, o CEL já trata o erro como o
enunciado pede: `false && erro` dá falso e `true || erro` dá verdadeiro.

Expressão com erro de sintaxe, função inexistente ou que não devolve booleano falha na carga.

Leitura enxuta, como na solução Rust: os caminhos citados (`financeiro.valor`, `status`...) saem
da árvore de cada expressão CEL na carga, e o resto da linha é validado e descartado sem alocar.
Um teste diferencial com fuzzing ([documento_test.go](solucao-go_cel/documento_test.go)) confere
que o resultado é o mesmo do `json.Unmarshal` da linha inteira: mesmas linhas aceitas e
recusadas, mesmos valores, mesma regra. No set grande, 2,1 s → 0,74 s. O perfil mostrou 72% do
tempo no `encoding/json`. Depois disso o ganho veio de alocar menos: textos sem cópia, mapas
reaproveitados entre linhas e GC menos frequente (`SetGCPercent(800)`).

## Como reproduzir

Requisitos: Docker e Python 3 (só para o `medir.py` e os scripts de teste).

```bash
# Imagens
docker build -t motor .                                   # Rust (oficial)
docker build -f solucao-go_cel/Dockerfile -t motor-go .   # Go + CEL

# Testes automatizados (unitários + 109 casos de borda), dentro do Docker
docker build --target teste --progress=plain .
docker build -f solucao-go_cel/Dockerfile --target teste --progress=plain .

# Medição nos sets do kit, com 1 CPU como a banca (troque motor por motor-go para a outra solução)
cd hackathon-2026-d1-kit
python3 medir.py --set pequeno --cmd "docker run --rm --cpus=1 -v {dir}:/dados motor --entrada /dados/{arquivo}"
python3 medir.py --set grande  --cmd "docker run --rm --cpus=1 -v {dir}:/dados motor --entrada /dados/{arquivo}"
cd ..

# Casos de borda e massa suja conferida contra o oráculo
python3 hackathon-2026-d1-kit/medir.py --entrada testes/bordas.jsonl --gabarito testes/bordas_gabarito.txt \
  --cmd "docker run --rm --cpus=1 -v {dir}:/dados motor --entrada /dados/{arquivo}"
python3 testes/sujar.py 200000 1 /tmp/sujo.jsonl
python3 testes/oraculo.py /tmp/sujo.jsonl > /tmp/sujo_gabarito.txt
python3 hackathon-2026-d1-kit/medir.py --entrada /tmp/sujo.jsonl --gabarito /tmp/sujo_gabarito.txt \
  --cmd "docker run --rm --cpus=1 -v {dir}:/dados motor --entrada /dados/{arquivo}"
```

Sem o `medir.py`: `docker run --rm --cpus=1 -v "$PWD/dados":/dados motor --entrada /dados/entrada.jsonl > saida.txt`.

## Evidências

Todas medidas com `medir.py`, Docker e `--cpus=1`, num Apple M5 Pro. Mediana de 3 execuções.

| Conjunto | Casos | Rust | Go + CEL |
|---|---|---|---|
| Pequeno (kit) | 100 | 100% · 0,175 s | 100% · 0,176 s |
| Grande (kit) | 200.000 | 100% · 0,453 s | 100% · 0,735 s |
| Bordas (escrito à mão a partir do enunciado) | 109 | 100% · 0,177 s | 100% · 0,184 s |
| Sujos × oráculo (3 sementes) | 600.000 | 100% · ~0,50 s cada | 100% · ~0,75 s cada |

Arquivos: [evidencias/rust/](evidencias/rust/) e [evidencias/go_cel/](evidencias/go_cel/).

### Por que testar além do kit

Os dois sets do kit são limpos: todo campo vem com o tipo esperado. O set surpresa testa
justamente campo ausente, tipo errado, valor no limite e conflito de ordem. Por isso há duas
camadas de teste a mais, compartilhadas pelas duas soluções em [testes/](testes/):

1. **Bordas** ([gerar_bordas.py](testes/gerar_bordas.py)): 109 casos, um por armadilha, com o
   resultado esperado escrito à mão a partir do enunciado, nunca copiado de um motor.
2. **Oráculo** ([oraculo.py](testes/oraculo.py)): as 12 regras reescritas direto em Python, sem
   ler nenhum arquivo de regras. Ele confere os motores em 600 mil registros do set grande
   "sujados" por [sujar.py](testes/sujar.py). Assim pega tanto erro de implementação quanto erro
   de tradução das regras.

### O que foi medido e descartado

| Ideia | Resultado |
|---|---|
| Paralelismo (a ordem da saída é livre) | sem ganho: a banca usa 1 CPU e o motor é ~90% CPU; threads dividem a mesma cota |
| Parser JSON com SIMD (`sonic-rs`) | 0,45 → 0,41 s no Mac, mas no x86 o SIMD exige AVX2 em tempo de compilação: sem ele o ganho some, com ele o motor quebra em máquina sem AVX2. Revertido |
| Reduzir os ~0,17 s do Docker | não dá: rede, criação do container e `--rm` vêm dos flags da banca |

## Onde a IA errou

As duas soluções foram construídas com o Claude Code. O que precisou de correção:

1. **`NaN` virava texto.** Para não perder a linha, a IA trocava `NaN` por uma string. Num campo
   de texto, como `uf_origem: NaN`, isso passava como texto válido e disparava a R08 por engano.
   O oráculo apontou 0,22% de divergência na massa suja. Correção: trocar por `[]`, que não é de
   nenhum tipo esperado nem "vazio". Dois casos de borda foram adicionados para isso não voltar.
2. **Perguntas que o enunciado já respondia.** Na análise, a IA listou como "questões em aberto"
   casos que a seção de semântica já resolve (por exemplo, número em texto). Correção minha:
   seguir o enunciado ao pé da letra.
3. **Bônus incompatível com o contrato.** A IA sugeriu mostrar na saída todas as regras que
   disparam, sem checar que o `medir.py` só aceita a linha exata `id:X - regra: Y`. O bônus saiu
   do escopo.
4. **Otimização medida só no Mac.** A IA mediu o ganho do parser SIMD no Mac (ARM), onde o SIMD é
   de série. Ao checar o código da biblioteca, viu que no x86 ele depende de AVX2 em tempo de
   compilação. Sem essa checagem, o ganho teria ido para o README sem valer na máquina da banca.
5. **Escapes perdidos ao editar por script.** Uma edição feita via script Python removeu as barras
   de um literal de teste em Rust. Revisei o diff e corrigi antes de rodar.

O próprio oráculo tem um limite: foi escrito pela mesma IA, então uma leitura errada do
enunciado poderia aparecer nele e nos motores. Os casos de borda escritos à mão cobrem esse risco.

## Limitações conhecidas

- Rust: número fora da faixa de `f64` (por exemplo `1e400`) faz o parser recusar a linha, que é
  ignorada com aviso no stderr.
- Go + CEL: o `id` decimal (`1.0`) sai como `1`, porque o Go lê todo número como `float64`. Os ids
  do contrato são inteiros.
