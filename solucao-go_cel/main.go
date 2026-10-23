// motor --entrada entrada.jsonl [--regras regras/duplicatas.json] > saida.txt
package main

import (
	"bufio"
	"bytes"
	"errors"
	"fmt"
	"math"
	"os"
	"runtime/debug"
	"strconv"
)

const regrasPadrao = "regras/duplicatas.json"

func main() {
	if err := executar(); err != nil {
		fmt.Fprintln(os.Stderr, "erro:", err)
		os.Exit(2)
	}
}

func executar() error {
	// Cada linha gera lixo pequeno e quase nada sobrevive: coletar com menos frequência
	// rende ~10% no set grande (medido com GOGC 100, 400, 800 e off; off fica mais lento).
	debug.SetGCPercent(800)
	entrada, arquivoRegras, err := argumentos()
	if err != nil {
		return err
	}
	motor, err := Carregar(arquivoRegras)
	if err != nil {
		return fmt.Errorf("%s: %w", arquivoRegras, err)
	}
	f, err := os.Open(entrada)
	if err != nil {
		return err
	}
	defer f.Close()
	// O Scanner devolve a linha no próprio buffer, sem cópia: o documento lido dela só vale
	// até a próxima linha, e processar termina antes disso.
	leitor := bufio.NewScanner(f)
	leitor.Buffer(make([]byte, 1<<20), math.MaxInt32)
	saida := bufio.NewWriterSize(os.Stdout, 1<<20)
	defer saida.Flush()

	for numero := 1; leitor.Scan(); numero++ {
		processar(motor, leitor.Bytes(), numero, saida)
	}
	return leitor.Err()
}

func processar(motor *Motor, linha []byte, numero int, saida *bufio.Writer) {
	// BOM do UTF-8 no início do arquivo não faz parte do JSON.
	linha = bytes.TrimSpace(bytes.TrimPrefix(linha, []byte("\xEF\xBB\xBF")))
	if len(linha) == 0 {
		return
	}
	// Fora do contrato (JSON inválido ou sem id): avisa no stderr e segue.
	doc, ok := motor.Ler(linha)
	if !ok {
		fmt.Fprintf(os.Stderr, "aviso: linha %d: JSON inválido, ignorada\n", numero)
		return
	}
	var id string
	switch v := doc["id"].(type) {
	case float64:
		if v == math.Trunc(v) && math.Abs(v) < 1e15 {
			id = strconv.FormatInt(int64(v), 10)
		} else {
			id = strconv.FormatFloat(v, 'g', -1, 64)
		}
	case string:
		id = v
	default:
		fmt.Fprintf(os.Stderr, "aviso: linha %d: sem id, ignorada\n", numero)
		return
	}
	codigo := motor.Classificar(doc)
	saida.WriteString("id:")
	saida.WriteString(id)
	saida.WriteString(" - regra: ")
	saida.WriteString(codigo)
	saida.WriteByte('\n')
}

func argumentos() (string, string, error) {
	uso := errors.New("uso: motor --entrada <arquivo.jsonl> [--regras <regras.json>]")
	entrada, regras := "", regrasPadrao
	args := os.Args[1:]
	for i := 0; i < len(args); i++ {
		if i+1 >= len(args) {
			return "", "", uso
		}
		switch args[i] {
		case "--entrada":
			entrada = args[i+1]
		case "--regras":
			regras = args[i+1]
		default:
			return "", "", uso
		}
		i++
	}
	if entrada == "" {
		return "", "", uso
	}
	return entrada, regras, nil
}
