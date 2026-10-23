#!/usr/bin/env python3
"""Gera uma massa "suja" a partir do set grande do kit: campo ausente, nulo, tipo trocado,
valor no limite, texto com caixa ou espaço diferente, data inválida, NaN, pai que não é objeto.
Também força valores que disparam regras, para criar conflito de ordem.

Uso: python3 testes/sujar.py <quantidade> <semente> <saida.jsonl>
Depois: python3 testes/oraculo.py <saida.jsonl> > <gabarito.txt>
"""
import gzip, json, os, random, sys

KIT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "hackathon-2026-d1-kit")

DISPARADORES = {
    "status": ["ATIVA", "LIQUIDADA", "CANCELADA"],
    "logistica.status_entrega": ["ENTREGUE", "EM_TRANSITO", "DEVOLVIDA", "AVARIADA"],
    "manifestacao.tipo": ["ACTO", "RCAV", "RCQQ", "RCVN", "RCVL", "RCOP", "ANTC", None, ""],
    "financeiro.tipo_negociacao": ["CSCO", "CSAO", "CSFD", "PENR", "PRCS", None],
    "financeiro.valor": [100000, 100000.0, 99999.99, 500000, 499999.99, 200000, 199999.99, 5000, 5000.01, 0, -1],
    "financeiro.valor_pago": [0, 0.0],
    "sacado.segmento": ["AGRO", "VAREJO"],
    "logistica.uf_destino": ["SP", "SC", "RJ"],
    "fiscal.cfop": ["5102", "6102", "5", "5405"],
    "logistica.modal": ["AEREO", "RODOVIARIO"],
    "logistica.peso_kg": [500, 500.0, 500.01, 499.99, 10000],
    "logistica.migo": [True, False],
    "pagamento.instrumento": ["PPIX", "BOLETO", "TRCR"],
    "sacador.porte": ["ME", "EPP", "MEDIA", "GRANDE"],
    "data_vencimento": ["2026-10-22", "2026-10-23", "2026-10-24", "2024-02-29", "2026-01-01"],
}
PAIS = ["sacador", "sacado", "financeiro", "pagamento", "fiscal", "logistica", "manifestacao"]
DATAS_RUINS = ["2026-02-30", "2025-02-29", "2026-13-01", "2026-00-10", "2026-9-30", "30/09/2026",
               "2026-09-30T00:00:00", " 2026-09-30", "", "0000-01-01", "2026-04-31", 20260930]


def folhas(doc, prefixo=""):
    for k, v in doc.items():
        if isinstance(v, dict):
            yield from folhas(v, f"{prefixo}{k}.")
        elif k != "id":
            yield f"{prefixo}{k}"


def definir(doc, caminho, valor, apagar=False):
    partes = caminho.split(".")
    for p in partes[:-1]:
        if not isinstance(doc.get(p), dict):
            doc[p] = {}
        doc = doc[p]
    if apagar:
        doc.pop(partes[-1], None)
    else:
        doc[partes[-1]] = valor


def obter(doc, caminho):
    for p in caminho.split("."):
        if not isinstance(doc, dict):
            return None
        doc = doc.get(p)
    return doc


def sujar(doc, rnd):
    for _ in range(rnd.randint(1, 4)):
        tipo = rnd.random()
        if tipo < 0.45:
            caminho = rnd.choice(list(DISPARADORES))
            definir(doc, caminho, rnd.choice(DISPARADORES[caminho]))
            continue
        caminho = rnd.choice(list(folhas(doc)) + list(DISPARADORES))
        atual = obter(doc, caminho)
        opcoes = [
            lambda: definir(doc, caminho, None, apagar=True),
            lambda: definir(doc, caminho, None),
            lambda: definir(doc, caminho, str(atual)),
            lambda: definir(doc, caminho, ""),
            lambda: definir(doc, caminho, rnd.choice([True, False, "true", "false", 0, 1])),
            lambda: definir(doc, caminho, float(rnd.choice(["nan", "inf", "-inf"]))),
        ]
        if isinstance(atual, str):
            opcoes += [lambda: definir(doc, caminho, atual.lower()), lambda: definir(doc, caminho, atual + " "),
                       lambda: definir(doc, caminho, " " + atual)]
            if atual.isdigit():
                opcoes.append(lambda: definir(doc, caminho, int(atual)))
        if caminho.startswith("data") or caminho.endswith(".data") or caminho.endswith("data_entrega"):
            opcoes.append(lambda: definir(doc, caminho, rnd.choice(DATAS_RUINS)))
        if tipo > 0.95:
            pai = rnd.choice(PAIS)
            opcoes = [lambda: doc.__setitem__(pai, rnd.choice(["x", [], None, 1])), lambda: doc.pop(pai, None)]
        rnd.choice(opcoes)()
    return doc


def main():
    quantidade, semente, destino = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
    rnd = random.Random(semente)
    with gzip.open(os.path.join(KIT, "dados", "grande", "entrada.jsonl.gz"), "rt", encoding="utf-8") as fi, \
            open(destino, "w", encoding="utf-8") as fo:
        for i, linha in enumerate(fi):
            if i >= quantidade:
                break
            doc = json.loads(linha)
            if rnd.random() < 0.8:
                sujar(doc, rnd)
            fo.write(json.dumps(doc, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
