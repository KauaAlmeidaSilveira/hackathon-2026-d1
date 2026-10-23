package main

// Leitura de uma linha do JSONL guardando só os campos que as regras citam (mesma ideia de
// solucao-rust/src/documento.rs). O esquema sai das expressões CEL, não do código; o resto da
// linha é validado e descartado sem alocar. Para o motor, o resultado é o mesmo de
// json.Unmarshal na linha inteira (conferido por teste diferencial em documento_test.go):
//   - chave repetida: vale a última; pai que não é objeto: filhos ausentes;
//   - NaN/Infinity (não são JSON válido) viram []: lista não é texto, número, booleano, data
//     nem "vazio";
//   - número fora do alcance do float64, em qualquer lugar da linha, invalida a linha.
//
// Para não alocar por linha, textos sem escape apontam para os bytes da linha e os objetos do
// esquema são reaproveitados de uma linha para a outra: o documento só vale até a próxima
// leitura, e a linha não pode mudar enquanto ele estiver em uso. Não é seguro entre goroutines.

import (
	"bytes"
	"encoding/json"
	"strconv"
	"unsafe"

	"github.com/google/cel-go/common/ast"
)

// esquema é a árvore dos caminhos citados. Nó `inteiro`: a expressão usa o valor todo
// (folha de um caminho), então tudo abaixo dele é lido.
type esquema struct {
	filhos  map[string]*esquema
	inteiro bool
	mapa    map[string]any // reaproveitado a cada linha
}

func (e *esquema) registrar(caminho []string) {
	no := e
	for _, parte := range caminho {
		if no.filhos == nil {
			no.filhos = map[string]*esquema{}
		}
		if no.filhos[parte] == nil {
			no.filhos[parte] = &esquema{}
		}
		no = no.filhos[parte]
	}
	no.inteiro = true
}

// registrarExpressao guarda cada cadeia `a.b.c` da expressão. Identificador ou cadeia usados
// fora de outra seleção (ex.: `vazio(financeiro)`) são lidos inteiros.
func (e *esquema) registrarExpressao(raiz ast.Expr) {
	operandos := map[int64]bool{}
	ast.PreOrderVisit(raiz, ast.NewExprVisitor(func(x ast.Expr) {
		if x.Kind() == ast.SelectKind {
			operandos[x.AsSelect().Operand().ID()] = true
		}
	}))
	ast.PreOrderVisit(raiz, ast.NewExprVisitor(func(x ast.Expr) {
		if operandos[x.ID()] {
			return
		}
		if caminho, ok := cadeia(x); ok {
			e.registrar(caminho)
		}
	}))
}

func cadeia(x ast.Expr) ([]string, bool) {
	switch x.Kind() {
	case ast.IdentKind:
		return []string{x.AsIdent()}, true
	case ast.SelectKind:
		s := x.AsSelect()
		pai, ok := cadeia(s.Operand())
		return append(pai, s.FieldName()), ok
	}
	return nil, false
}

const (
	pular   = iota // valida e descarta
	filtrar        // objeto: lê só as chaves do esquema
	ler            // lê tudo
)

// Mesmo limite de aninhamento do encoding/json.
const profundidadeMaxima = 10000

var naoNumero = []any{}

type leitor struct {
	b            []byte
	i            int
	profundidade int
}

// ler devolve false se a linha não for um objeto JSON válido.
func (e *esquema) ler(linha []byte) (map[string]any, bool) {
	p := leitor{b: linha}
	p.espacos()
	if p.i >= len(p.b) || p.b[p.i] != '{' {
		return nil, false
	}
	doc, ok := p.objeto(e, filtrar)
	p.espacos()
	if !ok || p.i != len(p.b) {
		return nil, false
	}
	return doc, true
}

func (p *leitor) espacos() {
	for p.i < len(p.b) {
		switch p.b[p.i] {
		case ' ', '\t', '\n', '\r':
			p.i++
		default:
			return
		}
	}
}

func (p *leitor) valor(no *esquema, modo int) (any, bool) {
	p.espacos()
	if p.i >= len(p.b) {
		return nil, false
	}
	switch p.b[p.i] {
	case '{':
		return p.objeto(no, modo)
	case '[':
		return p.lista(modo)
	case '"':
		s, crua, ok := p.texto(modo != pular)
		if crua != nil {
			s = emprestar(crua)
		}
		return s, ok
	case 't':
		return true, p.literal("true")
	case 'f':
		return false, p.literal("false")
	case 'n':
		return nil, p.literal("null")
	case 'N':
		return naoNumero, p.literal("NaN")
	case 'I':
		return naoNumero, p.literal("Infinity")
	case '-':
		if bytes.HasPrefix(p.b[p.i:], []byte("-Infinity")) {
			return naoNumero, p.literal("-Infinity")
		}
	}
	return p.numero(modo != pular)
}

func (p *leitor) literal(s string) bool {
	if !bytes.HasPrefix(p.b[p.i:], []byte(s)) {
		return false
	}
	p.i += len(s)
	return true
}

func (p *leitor) objeto(no *esquema, modo int) (map[string]any, bool) {
	if p.profundidade++; p.profundidade > profundidadeMaxima {
		return nil, false
	}
	defer func() { p.profundidade-- }()
	p.i++
	var m map[string]any
	switch modo {
	case filtrar:
		// Chave repetida reaproveita o mesmo mapa: vale a última, como no json.Unmarshal.
		if no.mapa == nil {
			no.mapa = make(map[string]any, len(no.filhos)+1) // +1: "variaveis", gravada pelo motor
		}
		m = no.mapa
		clear(m)
	case ler:
		m = map[string]any{}
	}
	p.espacos()
	if p.i < len(p.b) && p.b[p.i] == '}' {
		p.i++
		return m, true
	}
	for {
		p.espacos()
		if p.i >= len(p.b) || p.b[p.i] != '"' {
			return nil, false
		}
		chave, crua, ok := p.texto(modo != pular)
		if !ok {
			return nil, false
		}
		modoFilho, filho := modo, (*esquema)(nil)
		if modo == filtrar {
			if crua != nil {
				filho = no.filhos[string(crua)] // sem alocar
			} else {
				filho = no.filhos[chave]
			}
			switch {
			case filho == nil:
				modoFilho = pular
			case filho.inteiro:
				modoFilho = ler
			}
		}
		if modoFilho != pular && crua != nil {
			chave = emprestar(crua)
		}
		p.espacos()
		if p.i >= len(p.b) || p.b[p.i] != ':' {
			return nil, false
		}
		p.i++
		v, ok := p.valor(filho, modoFilho)
		if !ok {
			return nil, false
		}
		if modoFilho != pular {
			m[chave] = v
		}
		p.espacos()
		if p.i >= len(p.b) {
			return nil, false
		}
		switch p.b[p.i] {
		case ',':
			p.i++
		case '}':
			p.i++
			return m, true
		default:
			return nil, false
		}
	}
}

// lista: nenhuma regra seleciona dentro de lista, então ela é lida inteira ou pulada.
func (p *leitor) lista(modo int) ([]any, bool) {
	if p.profundidade++; p.profundidade > profundidadeMaxima {
		return nil, false
	}
	defer func() { p.profundidade-- }()
	p.i++
	if modo == filtrar {
		modo = ler
	}
	var l []any
	if modo == ler {
		l = []any{}
	}
	p.espacos()
	if p.i < len(p.b) && p.b[p.i] == ']' {
		p.i++
		return l, true
	}
	for {
		v, ok := p.valor(nil, modo)
		if !ok {
			return nil, false
		}
		if modo == ler {
			l = append(l, v)
		}
		p.espacos()
		if p.i >= len(p.b) {
			return nil, false
		}
		switch p.b[p.i] {
		case ',':
			p.i++
		case ']':
			p.i++
			return l, true
		default:
			return nil, false
		}
	}
}

// texto valida a string que começa em p.i. Sem escape e só ASCII, devolve os bytes crus (quem
// chama decide se aloca); senão, com decodificar, delega ao encoding/json (escapes, surrogates
// e UTF-8 inválido viram exatamente o que o json.Unmarshal produziria).
// comum: byte que não encerra, não escapa e não invalida um texto, nem o tira do caminho simples.
var comum = func() (t [256]bool) {
	for c := 0x20; c < 0x80; c++ {
		t[c] = c != '"' && c != '\\'
	}
	return t
}()

func (p *leitor) texto(decodificar bool) (string, []byte, bool) {
	inicio, simples := p.i, true
	for j := p.i + 1; j < len(p.b); j++ {
		for j < len(p.b) && comum[p.b[j]] {
			j++
		}
		if j >= len(p.b) {
			break
		}
		c := p.b[j]
		switch {
		case c == '"':
			p.i = j + 1
			if simples {
				return "", p.b[inicio+1 : j], true
			}
			if !decodificar {
				return "", nil, true
			}
			var s string
			if json.Unmarshal(p.b[inicio:p.i], &s) != nil {
				return "", nil, false
			}
			return s, nil, true
		case c < 0x20:
			return "", nil, false
		case c == '\\':
			simples = false
			if j+1 >= len(p.b) {
				return "", nil, false
			}
			j++
			switch p.b[j] {
			case '"', '\\', '/', 'b', 'f', 'n', 'r', 't':
			case 'u':
				if j+4 >= len(p.b) {
					return "", nil, false
				}
				for _, h := range p.b[j+1 : j+5] {
					if !(h >= '0' && h <= '9' || h >= 'a' && h <= 'f' || h >= 'A' && h <= 'F') {
						return "", nil, false
					}
				}
				j += 4
			default:
				return "", nil, false
			}
		case c >= 0x80:
			simples = false
		}
	}
	return "", nil, false
}

func emprestar(b []byte) string {
	return unsafe.String(unsafe.SliceData(b), len(b))
}

func (p *leitor) numero(decodificar bool) (any, bool) {
	inicio := p.i
	digitos := func() int {
		n := 0
		for p.i < len(p.b) && p.b[p.i] >= '0' && p.b[p.i] <= '9' {
			p.i++
			n++
		}
		return n
	}
	if p.i < len(p.b) && p.b[p.i] == '-' {
		p.i++
	}
	switch {
	case p.i < len(p.b) && p.b[p.i] == '0':
		p.i++
	case digitos() == 0:
		return nil, false
	}
	if p.i < len(p.b) && p.b[p.i] == '.' {
		p.i++
		if digitos() == 0 {
			return nil, false
		}
	}
	expoente := false
	if p.i < len(p.b) && (p.b[p.i] == 'e' || p.b[p.i] == 'E') {
		expoente = true
		p.i++
		if p.i < len(p.b) && (p.b[p.i] == '+' || p.b[p.i] == '-') {
			p.i++
		}
		if digitos() == 0 {
			return nil, false
		}
	}
	cru := p.b[inicio:p.i]
	// Sem expoente, só passa do alcance do float64 com mais de 300 dígitos.
	if !decodificar && !expoente && len(cru) <= 300 {
		return nil, true
	}
	n, err := strconv.ParseFloat(string(cru), 64)
	return n, err == nil
}
