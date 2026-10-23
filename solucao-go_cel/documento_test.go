package main

import (
	"bytes"
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"
)

// Referência: a leitura que a leitura enxuta substitui (json.Unmarshal da linha inteira).
// O esquema precisa dar o mesmo resultado nos campos citados e aceitar/recusar as mesmas linhas.
// lerLinha decodifica um objeto JSON. Se o parser recusar a linha por causa de NaN/Infinity
// (não são JSON válido), esses tokens viram [] e a linha é lida de novo: lista não é texto,
// número, booleano nem data, e não é "vazio".
func lerLinha(linha []byte) (map[string]any, bool) {
	var doc map[string]any
	if json.Unmarshal(linha, &doc) == nil && doc != nil {
		return doc, true
	}
	if reescrita, trocou := neutralizarNaoNumeros(linha); trocou {
		doc = nil
		if json.Unmarshal(reescrita, &doc) == nil && doc != nil {
			return doc, true
		}
	}
	return nil, false
}

func neutralizarNaoNumeros(linha []byte) ([]byte, bool) {
	tokens := [][]byte{[]byte("-Infinity"), []byte("Infinity"), []byte("NaN")}
	saida := make([]byte, 0, len(linha)+32)
	emTexto, trocou := false, false
	for i := 0; i < len(linha); i++ {
		c := linha[i]
		if emTexto {
			saida = append(saida, c)
			if c == '\\' && i+1 < len(linha) {
				i++
				saida = append(saida, linha[i])
			} else if c == '"' {
				emTexto = false
			}
			continue
		}
		achou := false
		for _, t := range tokens {
			if bytes.HasPrefix(linha[i:], t) {
				saida = append(saida, '[', ']')
				i += len(t) - 1
				trocou, achou = true, true
				break
			}
		}
		if !achou {
			emTexto = c == '"'
			saida = append(saida, c)
		}
	}
	return saida, trocou
}

// projetar recorta o documento completo nos caminhos do esquema, como a leitura enxuta faz.
func projetar(v any, no *esquema) any {
	m, ok := v.(map[string]any)
	if no.inteiro || !ok {
		return v
	}
	saida := map[string]any{}
	for k, filho := range no.filhos {
		if x, existe := m[k]; existe {
			saida[k] = projetar(x, filho)
		}
	}
	return saida
}

func conferir(t *testing.T, motor *Motor, linha []byte) {
	t.Helper()
	esperado, okEsperado := lerLinha(linha)
	obtido, okObtido := motor.Ler(linha)
	if okEsperado != okObtido {
		t.Fatalf("%q: válida? esperado %v, obtido %v", linha, okEsperado, okObtido)
	}
	if !okEsperado {
		return
	}
	if p := projetar(esperado, motor.esquema); !reflect.DeepEqual(p, any(obtido)) {
		t.Fatalf("%q:\nesperado %#v\nobtido   %#v", linha, p, obtido)
	}
	if e, o := motor.Classificar(esperado), motor.Classificar(obtido); e != o {
		t.Fatalf("%q: regra esperada %s, obtida %s", linha, e, o)
	}
}

func esquemaDe(t *testing.T, condicao string) *Motor {
	t.Helper()
	arq := t.TempDir() + "/r.json"
	conteudo, _ := json.Marshal(map[string]any{"sem_regra": "N", "regras": []any{map[string]string{"codigo": "X", "condicao": condicao}}})
	if err := os.WriteFile(arq, conteudo, 0o644); err != nil {
		t.Fatal(err)
	}
	m, err := Carregar(arq)
	if err != nil {
		t.Fatal(err)
	}
	return m
}

func TestLeSoOsCamposCitados(t *testing.T) {
	m := esquemaDe(t, "texto(a) == 'x' && numero(b.c) > 1.0 && vazio(b.d)")
	doc, ok := m.Ler([]byte(`{"id":7,"a":"x","z":[1,{"q":2}],"b":{"c":1.5,"d":null,"e":"y"}}`))
	esperado := map[string]any{"id": 7.0, "a": "x", "b": map[string]any{"c": 1.5, "d": nil}}
	if !ok || !reflect.DeepEqual(doc, esperado) {
		t.Fatalf("%#v", doc)
	}
}

func TestPaiQueNaoEObjetoEChaveRepetida(t *testing.T) {
	m := esquemaDe(t, "texto(a) == 'x' && numero(b.c) > 1.0")
	for _, l := range []string{`{"b":"x"}`, `{"b":[1]}`, `{}`, `{"a":1,"a":"x","b":{"c":1},"b":{"d":2}}`,
		`{"b":{"c":2,"c":"y"}}`, `{"\u0061":"x","b":{"\u0063":2}}`} {
		conferir(t, m, []byte(l))
	}
}

func TestUsoDoObjetoInteiro(t *testing.T) {
	m := esquemaDe(t, "vazio(b) || numero(b.c) > 1.0")
	doc, _ := m.Ler([]byte(`{"b":{"c":2,"d":[1,{"e":NaN}]}}`))
	if !reflect.DeepEqual(doc["b"], map[string]any{"c": 2.0, "d": []any{1.0, map[string]any{"e": []any{}}}}) {
		t.Fatalf("%#v", doc["b"])
	}
}

func TestLinhaInvalida(t *testing.T) {
	m := esquemaDe(t, "texto(a) == 'x'")
	for _, l := range []string{`{"a":}`, `[1,2]`, `{"a":1} lixo`, `null`, `{"a":01}`, `{"a":1.}`, `{"a":.5}`,
		`{"a":"x\q"}`, `{"a":"x` + "\x01" + `"}`, `{"z":1e400}`, `{"z":-1e400}`, `{"a":-NaN}`, `{"a":nan}`,
		`{"a":+1}`, `{"a":tru}`, `{"a":trueNaN}`, `{"a":"\u12"}`, `{"a":1,}`, `{,}`, `{"a" 1}`, `{"a":[1,]}`,
		strings.Repeat("[", 10001) + strings.Repeat("]", 10001), `{"z":` + strings.Repeat("[", 10000) + strings.Repeat("]", 10000) + `}`} {
		conferir(t, m, []byte(l))
	}
}

func TestBordasIguaisAoUnmarshal(t *testing.T) {
	m, err := Carregar("regras/duplicatas.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, l := range linhas(t, "../testes/bordas.jsonl") {
		conferir(t, m, bytes.TrimSpace([]byte(l)))
	}
}

func FuzzLeitura(f *testing.F) {
	m, err := Carregar("regras/duplicatas.json")
	if err != nil {
		f.Fatal(err)
	}
	for _, l := range []string{`{"id":1,"status":"ATIVA","financeiro":{"valor":1e5,"valor_pago":NaN}}`,
		`{"id":"a","manifestacao":{"tipo":"RC\u0041V"},"logistica":"x"}`, `{"id":2,"fiscal":{"cfop":5102}}`} {
		f.Add([]byte(l))
	}
	f.Fuzz(func(t *testing.T, linha []byte) { conferir(t, m, linha) })
}
