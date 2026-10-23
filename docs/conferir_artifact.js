// Confere que a lógica JS da página classifica igual ao gabarito.
// Uso: node docs/conferir_artifact.js <entrada.jsonl> <gabarito.txt> [limite]
const fs = require("fs");
const [entrada, gabarito, limite] = process.argv.slice(2);
const html = fs.readFileSync(__dirname + "/motor-passo-a-passo.html", "utf8");
const regras = html.match(/<script type="application\/json" id="regras-json">([\s\S]*?)<\/script>/)[1];
const codigo = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]).join("\n");

// DOM mínimo: só o que o script da página toca.
const el = () => ({ innerHTML: "", textContent: "", value: "", children: [], style: {}, appendChild(c) { this.children.push(c); },
  addEventListener() {}, setAttribute() {} });
const nos = { "regras-json": { textContent: regras } };
global.document = { getElementById: (id) => (nos[id] ||= el()), createElement: el };
global.window = global;
new Function(codigo)();

const esperado = new Map(fs.readFileSync(gabarito, "utf8").trim().split("\n").map((l) => [l.split(" - ")[0], l.trim()]));
let total = 0, erros = 0;
for (const linha of fs.readFileSync(entrada, "utf8").split("\n")) {
  if (!linha.trim()) continue;
  if (limite && total >= +limite) break;
  total++;
  const obtido = window.classificarLinha(linha.trim());
  const chave = obtido && obtido.split(" - ")[0];
  if (!obtido || esperado.get(chave) !== obtido) {
    if (erros++ < 5) console.log("divergência:", obtido, "esperado:", esperado.get(chave));
  }
}
console.log(`${total} linhas, ${erros} divergências`);
process.exit(erros ? 1 : 0);
