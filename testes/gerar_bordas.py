#!/usr/bin/env python3
"""Gera testes/bordas.jsonl e testes/bordas_gabarito.txt.

Cada caso parte de uma duplicata base que não satisfaz nenhuma regra e muda só o que o caso
testa. O resultado esperado foi escrito à mão a partir do ENUNCIADO (regras + semântica),
nunca copiado da saída do motor.

Uso: python3 testes/gerar_bordas.py
"""
import copy, json, os

BASE = {
    "iud": "61853939617693296424", "numero": "107021/6", "status": "ATIVA",
    "data_emissao": "2026-08-27", "data_vencimento": "2026-12-31",
    "sacador": {"cnpj": "76054485338705", "uf": "SC", "porte": "MEDIA"},
    "sacado": {"cnpj": "23663297118298", "uf": "SC", "segmento": "SAUDE"},
    "financeiro": {"valor": 27821.81, "valor_pago": 0.0, "moeda": "BRL", "parcela": 3,
                   "total_parcelas": 3, "tipo_negociacao": None, "taxa_juros_mensal": 2.09},
    "pagamento": {"instrumento": "BOLETO", "compe": "756", "tipo_conta": "CCOR"},
    "fiscal": {"chave_nfe": "57704238582046710595978217864422302149821411", "cfop": "5102"},
    "logistica": {"modal": "FERROVIARIO", "transportadora_cnpj": "74720862750238",
                  "uf_origem": "SC", "uf_destino": "SC", "peso_kg": 26769.2,
                  "status_entrega": "ENTREGUE", "data_entrega": "2026-09-09", "migo": True},
    "manifestacao": {"tipo": None, "data": None},
}
AUSENTE = object()

# (descrição, alterações {caminho: valor | AUSENTE}, esperado)
CASOS = [
    ("base não satisfaz nenhuma regra", {}, "SEM_REGRA"),

    # R01 · status, texto exato
    ("R01 LIQUIDADA", {"status": "LIQUIDADA"}, "R01"),
    ("R01 CANCELADA", {"status": "CANCELADA"}, "R01"),
    ("R01 minúsculo não vale", {"status": "liquidada"}, "SEM_REGRA"),
    ("R01 espaço no fim não vale", {"status": "CANCELADA "}, "SEM_REGRA"),
    ("R01 status ausente", {"status": AUSENTE}, "SEM_REGRA"),
    ("R01 vence R02 pela ordem", {"status": "LIQUIDADA", "logistica.status_entrega": "DEVOLVIDA"}, "R01"),

    # R02 · OU
    ("R02 DEVOLVIDA", {"logistica.status_entrega": "DEVOLVIDA"}, "R02"),
    ("R02 RCOP", {"manifestacao.tipo": "RCOP"}, "R02"),
    ("R02 vence R03 pela ordem", {"logistica.status_entrega": "DEVOLVIDA", "manifestacao.tipo": "RCAV"}, "R02"),
    ("R02 devolvida minúsculo", {"logistica.status_entrega": "devolvida"}, "SEM_REGRA"),

    # R03 · lista
    ("R03 RCAV", {"manifestacao.tipo": "RCAV"}, "R03"),
    ("R03 RCQQ", {"manifestacao.tipo": "RCQQ"}, "R03"),
    ("R03 RCVN", {"manifestacao.tipo": "RCVN"}, "R03"),
    ("R03 RCVL", {"manifestacao.tipo": "RCVL"}, "R03"),
    ("R03 minúsculo não vale", {"manifestacao.tipo": "rcav"}, "SEM_REGRA"),
    ("R03 vence R04 pela ordem", {"manifestacao.tipo": "RCAV", "logistica.status_entrega": "AVARIADA"}, "R03"),

    # R04 · "está vazio"
    ("R04 tipo nulo", {"logistica.status_entrega": "AVARIADA"}, "R04"),
    ("R04 tipo texto vazio", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": ""}, "R04"),
    ("R04 tipo ausente", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": AUSENTE}, "R04"),
    ("R04 manifestacao ausente", {"logistica.status_entrega": "AVARIADA", "manifestacao": AUSENTE}, "R04"),
    ("R04 manifestacao nula", {"logistica.status_entrega": "AVARIADA", "manifestacao": None}, "R04"),
    ("R04 tipo espaço não é vazio", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": " "}, "SEM_REGRA"),
    ("R04 tipo 0 não é vazio", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": 0}, "SEM_REGRA"),
    ("R04 tipo false não é vazio", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": False}, "SEM_REGRA"),
    ("R04 ACTO cai em R11", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": "ACTO"}, "R11"),

    # R05 · data e campo contra campo
    ("R05 vencida ontem", {"data_vencimento": "2026-10-22"}, "R05"),
    ("R05 vence hoje não é vencida", {"data_vencimento": "2026-10-23"}, "SEM_REGRA"),
    ("R05 pago um centavo a menos", {"data_vencimento": "2026-01-01", "financeiro.valor_pago": 27821.80}, "R05"),
    ("R05 pago igual", {"data_vencimento": "2026-01-01", "financeiro.valor_pago": 27821.81}, "SEM_REGRA"),
    ("R05 pago a mais", {"data_vencimento": "2026-01-01", "financeiro.valor_pago": 30000}, "SEM_REGRA"),
    ("R05 data inexistente", {"data_vencimento": "2026-02-30"}, "SEM_REGRA"),
    ("R05 29/02 em ano não bissexto", {"data_vencimento": "2025-02-29"}, "SEM_REGRA"),
    ("R05 29/02 em ano bissexto", {"data_vencimento": "2024-02-29"}, "R05"),
    ("R05 mês sem zero", {"data_vencimento": "2026-9-30"}, "SEM_REGRA"),
    ("R05 formato brasileiro", {"data_vencimento": "30/09/2026"}, "SEM_REGRA"),
    ("R05 data com hora", {"data_vencimento": "2026-09-30T00:00:00"}, "SEM_REGRA"),
    ("R05 data com espaço", {"data_vencimento": " 2026-09-30"}, "SEM_REGRA"),
    ("R05 data numérica", {"data_vencimento": 20260930}, "SEM_REGRA"),
    ("R05 data vazia", {"data_vencimento": ""}, "SEM_REGRA"),
    ("R05 data ausente", {"data_vencimento": AUSENTE}, "SEM_REGRA"),
    ("R05 mês 13", {"data_vencimento": "2026-13-01"}, "SEM_REGRA"),
    ("R05 valor_pago texto", {"data_vencimento": "2026-01-01", "financeiro.valor_pago": "0"}, "SEM_REGRA"),
    ("R05 valor_pago nulo", {"data_vencimento": "2026-01-01", "financeiro.valor_pago": None}, "SEM_REGRA"),
    ("R05 valor texto", {"data_vencimento": "2026-01-01", "financeiro.valor": "27821.81"}, "SEM_REGRA"),
    ("R05 financeiro ausente", {"data_vencimento": "2026-01-01", "financeiro": AUSENTE}, "SEM_REGRA"),
    ("R05 financeiro não é objeto", {"data_vencimento": "2026-01-01", "financeiro": "x"}, "SEM_REGRA"),
    ("R05 vence R06 pela ordem", {"data_vencimento": "2026-01-01", "financeiro.tipo_negociacao": "CSFD",
                                  "financeiro.valor": 150000}, "R05"),

    # R06
    ("R06 CSFD no limite", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": 100000}, "R06"),
    ("R06 PENR no limite decimal", {"financeiro.tipo_negociacao": "PENR", "financeiro.valor": 100000.0}, "R06"),
    ("R06 PENR abaixo do limite", {"financeiro.tipo_negociacao": "PENR", "financeiro.valor": 99999.99}, "SEM_REGRA"),
    ("R06 valor texto", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": "15000.00"}, "SEM_REGRA"),
    ("R06 valor texto alto", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": "150000"}, "SEM_REGRA"),
    ("R06 CSCO não vale", {"financeiro.tipo_negociacao": "CSCO", "financeiro.valor": 150000}, "SEM_REGRA"),
    ("R06 notação 1E5", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": 1e5}, "R06"),
    ("R06 vence R07 pela ordem", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": 600000}, "R06"),
    ("R06 valor NaN", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": float("nan")}, "SEM_REGRA"),
    ("R06 valor Infinity", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": float("inf")}, "SEM_REGRA"),
    ("R06 valor booleano", {"financeiro.tipo_negociacao": "CSFD", "financeiro.valor": True}, "SEM_REGRA"),

    # R07 · OU com parênteses
    ("R07 no limite", {"financeiro.valor": 500000}, "R07"),
    ("R07 abaixo do limite", {"financeiro.valor": 499999.99}, "SEM_REGRA"),
    ("R07 AGRO no limite", {"sacado.segmento": "AGRO", "financeiro.valor": 200000}, "R07"),
    ("R07 AGRO abaixo", {"sacado.segmento": "AGRO", "financeiro.valor": 199999.99}, "SEM_REGRA"),
    ("R07 agro minúsculo", {"sacado.segmento": "agro", "financeiro.valor": 200000}, "SEM_REGRA"),
    ("R07 não AGRO 300 mil", {"sacado.segmento": "VAREJO", "financeiro.valor": 300000}, "SEM_REGRA"),
    ("R07 AGRO sem valor", {"sacado.segmento": "AGRO", "financeiro.valor": AUSENTE}, "SEM_REGRA"),
    ("R07 notação 5e5", {"financeiro.valor": 5e5}, "R07"),

    # R08 · != e "começa com"
    ("R08 interestadual CFOP 5", {"logistica.uf_destino": "SP"}, "R08"),
    ("R08 CFOP 6", {"logistica.uf_destino": "SP", "fiscal.cfop": "6102"}, "SEM_REGRA"),
    ("R08 CFOP numérico", {"logistica.uf_destino": "SP", "fiscal.cfop": 5102}, "SEM_REGRA"),
    ("R08 CFOP só 5", {"logistica.uf_destino": "SP", "fiscal.cfop": "5"}, "R08"),
    ("R08 CFOP vazio", {"logistica.uf_destino": "SP", "fiscal.cfop": ""}, "SEM_REGRA"),
    ("R08 CFOP com espaço antes", {"logistica.uf_destino": "SP", "fiscal.cfop": " 5102"}, "SEM_REGRA"),
    ("R08 destino nulo", {"logistica.uf_destino": None}, "SEM_REGRA"),
    ("R08 destino ausente", {"logistica.uf_destino": AUSENTE}, "SEM_REGRA"),
    ("R08 origem ausente", {"logistica.uf_origem": AUSENTE}, "SEM_REGRA"),
    ("R08 destino numérico", {"logistica.uf_destino": 35}, "SEM_REGRA"),
    ("R08 caixa diferente conta como diferente", {"logistica.uf_destino": "sc"}, "R08"),
    ("R08 destino vazio", {"logistica.uf_destino": ""}, "R08"),
    ("R08 fiscal ausente", {"logistica.uf_destino": "SP", "fiscal": AUSENTE}, "SEM_REGRA"),
    ("R08 origem NaN não é texto", {"logistica.uf_origem": float("nan"), "logistica.uf_destino": "SP"}, "SEM_REGRA"),
    ("R04 tipo NaN não é vazio", {"logistica.status_entrega": "AVARIADA", "manifestacao.tipo": float("nan")}, "SEM_REGRA"),
    ("logistica não é objeto", {"logistica": "x"}, "SEM_REGRA"),

    # R09
    ("R09 peso no limite", {"logistica.modal": "AEREO", "logistica.peso_kg": 500}, "SEM_REGRA"),
    ("R09 peso acima", {"logistica.modal": "AEREO", "logistica.peso_kg": 500.01}, "R09"),
    ("R09 peso texto", {"logistica.modal": "AEREO", "logistica.peso_kg": "600"}, "SEM_REGRA"),
    ("R09 aereo minúsculo", {"logistica.modal": "aereo", "logistica.peso_kg": 600}, "SEM_REGRA"),
    ("R09 vence R10 pela ordem", {"logistica.modal": "AEREO", "logistica.peso_kg": 600, "logistica.migo": False}, "R09"),

    # R10 · booleano estrito
    ("R10 migo false", {"logistica.migo": False}, "R10"),
    ("R10 migo texto false", {"logistica.migo": "false"}, "SEM_REGRA"),
    ("R10 migo 0", {"logistica.migo": 0}, "SEM_REGRA"),
    ("R10 migo nulo", {"logistica.migo": None}, "SEM_REGRA"),
    ("R10 migo ausente", {"logistica.migo": AUSENTE}, "SEM_REGRA"),
    ("R10 EM_TRANSITO", {"logistica.migo": False, "logistica.status_entrega": "EM_TRANSITO"}, "SEM_REGRA"),
    ("R10 vence R11 pela ordem", {"logistica.migo": False, "manifestacao.tipo": "ACTO"}, "R10"),

    # R11
    ("R11 ACTO migo true", {"manifestacao.tipo": "ACTO"}, "R11"),
    ("R11 migo 1", {"manifestacao.tipo": "ACTO", "logistica.migo": 1}, "SEM_REGRA"),
    ("R11 migo texto true", {"manifestacao.tipo": "ACTO", "logistica.migo": "true"}, "SEM_REGRA"),
    ("R11 ANTC não vale", {"manifestacao.tipo": "ANTC"}, "SEM_REGRA"),
    ("R11 em trânsito também vale", {"manifestacao.tipo": "ACTO", "logistica.status_entrega": "EM_TRANSITO"}, "R11"),

    # R12 · três condições
    ("R12 ME no limite", {"pagamento.instrumento": "PPIX", "sacador.porte": "ME", "financeiro.valor": 5000}, "R12"),
    ("R12 EPP", {"pagamento.instrumento": "PPIX", "sacador.porte": "EPP", "financeiro.valor": 4999.99}, "R12"),
    ("R12 acima do limite", {"pagamento.instrumento": "PPIX", "sacador.porte": "ME", "financeiro.valor": 5000.01}, "SEM_REGRA"),
    ("R12 porte MEDIA", {"pagamento.instrumento": "PPIX", "sacador.porte": "MEDIA", "financeiro.valor": 100}, "SEM_REGRA"),
    ("R12 PIX não é PPIX", {"pagamento.instrumento": "PIX", "sacador.porte": "ME", "financeiro.valor": 100}, "SEM_REGRA"),
    ("R12 valor negativo", {"pagamento.instrumento": "PPIX", "sacador.porte": "ME", "financeiro.valor": -1}, "R12"),
    ("R12 valor texto", {"pagamento.instrumento": "PPIX", "sacador.porte": "ME", "financeiro.valor": "100"}, "SEM_REGRA"),
    ("R12 sacador ausente", {"pagamento.instrumento": "PPIX", "sacador": AUSENTE, "financeiro.valor": 100}, "SEM_REGRA"),

    # Campos extras não interferem
    ("campo extra ignorado", {"extra": {"qualquer": [1, 2]}}, "SEM_REGRA"),
]


def aplicar(doc, caminho, valor):
    partes = caminho.split(".")
    alvo = doc
    for p in partes[:-1]:
        alvo = alvo[p]
    if valor is AUSENTE:
        alvo.pop(partes[-1], None)
    else:
        alvo[partes[-1]] = valor


def main():
    aqui = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(aqui, "bordas.jsonl"), "w") as fe, open(os.path.join(aqui, "bordas_gabarito.txt"), "w") as fg:
        for i, (descricao, alteracoes, esperado) in enumerate(CASOS, start=1):
            doc = {"id": i, **copy.deepcopy(BASE)}
            for caminho, valor in alteracoes.items():
                aplicar(doc, caminho, valor)
            fe.write(json.dumps(doc, ensure_ascii=False) + "\n")  # allow_nan: escreve NaN/Infinity
            fg.write(f"id:{i} - regra: {esperado}\n")
    with open(os.path.join(aqui, "bordas_casos.txt"), "w") as fd:
        for i, (descricao, _, esperado) in enumerate(CASOS, start=1):
            fd.write(f"{i}\t{esperado}\t{descricao}\n")
    print(f"{len(CASOS)} casos gerados")


if __name__ == "__main__":
    main()
