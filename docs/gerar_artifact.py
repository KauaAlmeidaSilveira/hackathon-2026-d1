#!/usr/bin/env python3
"""Gera docs/motor-passo-a-passo.html embutindo o regras/duplicatas.json atual no template.

Rodar sempre que as regras ou a lógica do motor mudarem, e republicar o Artifact.
Uso: python3 docs/gerar_artifact.py
"""
import json, os

RAIZ = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
with open(os.path.join(RAIZ, "solucao-rust", "regras", "duplicatas.json"), encoding="utf-8") as f:
    regras = json.dumps(json.load(f), ensure_ascii=False).replace("</", "<\\/")
with open(os.path.join(RAIZ, "docs", "motor-passo-a-passo.template.html"), encoding="utf-8") as f:
    pagina = f.read().replace("__REGRAS__", regras)
with open(os.path.join(RAIZ, "docs", "motor-passo-a-passo.html"), "w", encoding="utf-8") as f:
    f.write(pagina)
print("docs/motor-passo-a-passo.html gerado")
