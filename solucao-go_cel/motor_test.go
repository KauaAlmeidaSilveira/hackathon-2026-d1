package main

import (
	"bufio"
	"os"
	"strconv"
	"strings"
	"testing"
)

// Roda o motor com as regras reais sobre ../testes/bordas.jsonl (compartilhado entre as
// soluções) e compara com o gabarito escrito à mão a partir do enunciado.
func TestBordas(t *testing.T) {
	motor, err := Carregar("regras/duplicatas.json")
	if err != nil {
		t.Fatal(err)
	}
	gabarito := map[string]string{}
	for _, l := range linhas(t, "../testes/bordas_gabarito.txt") {
		id, regra, _ := strings.Cut(strings.TrimPrefix(l, "id:"), " - regra: ")
		gabarito[id] = regra
	}
	casos := map[string]string{}
	for _, l := range linhas(t, "../testes/bordas_casos.txt") {
		id, resto, _ := strings.Cut(l, "\t")
		casos[id] = resto
	}
	if len(gabarito) == 0 {
		t.Fatal("gabarito vazio")
	}
	for _, l := range linhas(t, "../testes/bordas.jsonl") {
		doc, ok := motor.Ler([]byte(l))
		if !ok {
			t.Fatalf("linha ilegível: %s", l)
		}
		id := strconv.FormatInt(int64(doc["id"].(float64)), 10)
		if obtido := motor.Classificar(doc); obtido != gabarito[id] {
			t.Errorf("id %s (%s): obtido %s", id, casos[id], obtido)
		}
	}
}

func TestData(t *testing.T) {
	validas := map[string]int64{"2026-10-23": 20261023, "2024-02-29": 20240229, "2000-02-29": 20000229}
	for s, n := range validas {
		if got, ok := dataAAAAMMDD(s); !ok || got != n {
			t.Errorf("%s: %d %v", s, got, ok)
		}
	}
	for _, s := range []string{"2026-02-29", "2026-02-30", "1900-02-29", "0000-01-01", "2026-13-01", "2026-00-10",
		"2026-04-31", "2026-1-01", "2026/10/23", "2026-10-23T00:00:00", " 2026-10-2", ""} {
		if _, ok := dataAAAAMMDD(s); ok {
			t.Errorf("%q deveria ser inválida", s)
		}
	}
}

func TestRegraInvalidaFalhaNaCarga(t *testing.T) {
	for _, cond := range []string{"texto(status) ==", "funcao_que_nao_existe(status)", "numero(valor)", "texto(a, b) == 'x'"} {
		arq := t.TempDir() + "/r.json"
		os.WriteFile(arq, []byte(`{"sem_regra":"N","regras":[{"codigo":"X","condicao":"`+cond+`"}]}`), 0o644)
		if _, err := Carregar(arq); err == nil {
			t.Errorf("%q deveria falhar na carga", cond)
		}
	}
}

func linhas(t *testing.T, caminho string) []string {
	f, err := os.Open(caminho)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var out []string
	s := bufio.NewScanner(f)
	s.Buffer(make([]byte, 1<<20), 1<<20)
	for s.Scan() {
		if strings.TrimSpace(s.Text()) != "" {
			out = append(out, s.Text())
		}
	}
	return out
}
