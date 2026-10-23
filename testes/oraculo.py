#!/usr/bin/env python3
"""Oráculo: as 12 regras do ENUNCIADO escritas direto em Python, de forma independente do motor.

Não lê regras/duplicatas.json de propósito: assim pega erro de tradução das regras para o JSON
e erro de implementação do motor. Só serve para teste; é lento e não é o motor.

Uso: python3 testes/oraculo.py entrada.jsonl > gabarito.txt
"""
import datetime, json, math, re, sys

AUSENTE = object()
REFERENCIA = datetime.date(2026, 10, 23)
DATA = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")


def campo(doc, caminho):
    for parte in caminho.split("."):
        if not isinstance(doc, dict) or parte not in doc:
            return AUSENTE
        doc = doc[parte]
    return doc


def texto(v):
    return v if isinstance(v, str) else None


def numero(v):
    # bool é subclasse de int em Python: type() exclui. NaN/Infinity não são número JSON.
    return v if type(v) in (int, float) and math.isfinite(v) else None


def booleano(v):
    return v if type(v) is bool else None


def data(v):
    if not isinstance(v, str) or not DATA.fullmatch(v):
        return None
    try:
        return datetime.date(int(v[0:4]), int(v[5:7]), int(v[8:10]))
    except ValueError:
        return None


def vazio(v):
    return v is AUSENTE or v is None or v == ""


def classificar(d):
    t = lambda c: texto(campo(d, c))
    n = lambda c: numero(campo(d, c))
    b = lambda c: booleano(campo(d, c))
    valor, pago = n("financeiro.valor"), n("financeiro.valor_pago")
    venc = data(campo(d, "data_vencimento"))
    origem, destino, cfop = t("logistica.uf_origem"), t("logistica.uf_destino"), t("fiscal.cfop")

    if t("status") in ("CANCELADA", "LIQUIDADA"):
        return "R01"
    if t("logistica.status_entrega") == "DEVOLVIDA" or t("manifestacao.tipo") == "RCOP":
        return "R02"
    if t("manifestacao.tipo") in ("RCAV", "RCQQ", "RCVN", "RCVL"):
        return "R03"
    if t("logistica.status_entrega") == "AVARIADA" and vazio(campo(d, "manifestacao.tipo")):
        return "R04"
    if venc is not None and venc < REFERENCIA and pago is not None and valor is not None and pago < valor:
        return "R05"
    if t("financeiro.tipo_negociacao") in ("CSFD", "PENR") and valor is not None and valor >= 100000:
        return "R06"
    if valor is not None and (valor >= 500000 or (t("sacado.segmento") == "AGRO" and valor >= 200000)):
        return "R07"
    if origem is not None and destino is not None and origem != destino and cfop is not None and cfop.startswith("5"):
        return "R08"
    peso = n("logistica.peso_kg")
    if t("logistica.modal") == "AEREO" and peso is not None and peso > 500:
        return "R09"
    if t("logistica.status_entrega") == "ENTREGUE" and b("logistica.migo") is False:
        return "R10"
    if t("manifestacao.tipo") == "ACTO" and b("logistica.migo") is True:
        return "R11"
    if (t("pagamento.instrumento") == "PPIX" and t("sacador.porte") in ("ME", "EPP")
            and valor is not None and valor <= 5000):
        return "R12"
    return "SEM_REGRA"


def main():
    saida = sys.stdout
    with open(sys.argv[1], encoding="utf-8") as f:
        for linha in f:
            if linha.strip():
                d = json.loads(linha)
                saida.write(f"id:{d['id']} - regra: {classificar(d)}\n")


if __name__ == "__main__":
    main()
