package main

// Motor genérico: as regras são expressões CEL lidas de um arquivo JSON.
// O código não conhece nenhuma regra de negócio; só a semântica do ENUNCIADO, exposta como
// funções CEL: texto(), numero(), booleano(), data() e vazio().
//
// Semântica: em CEL, campo ausente (ou pai que não é objeto) vira erro. As funções de tipo
// também devolvem erro quando o valor não é do tipo pedido. Uma regra que termina em erro
// conta como falsa. Com && e ||, CEL já trata o erro como o enunciado pede:
// `false && erro` = false, `true || erro` = true, e `true && erro` = erro (falsa).

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/google/cel-go/cel"
	"github.com/google/cel-go/common/ast"
	"github.com/google/cel-go/common/types"
	"github.com/google/cel-go/common/types/ref"
	"github.com/google/cel-go/interpreter"
)

type Motor struct {
	regras    []regra
	semRegra  string
	variaveis map[string]any
	esquema   *esquema // campos que as regras citam: o resto da linha não é decodificado
}

type regra struct {
	codigo   string
	programa cel.Program
}

type arquivoRegras struct {
	Variaveis map[string]any `json:"variaveis"`
	SemRegra  string         `json:"sem_regra"`
	Regras    []struct {
		Codigo   string `json:"codigo"`
		Condicao string `json:"condicao"`
	} `json:"regras"`
}

func Carregar(caminho string) (*Motor, error) {
	bruto, err := os.ReadFile(caminho)
	if err != nil {
		return nil, err
	}
	var arq arquivoRegras
	if err := json.Unmarshal(bruto, &arq); err != nil {
		return nil, fmt.Errorf("JSON inválido: %w", err)
	}
	if arq.SemRegra == "" || len(arq.Regras) == 0 {
		return nil, fmt.Errorf("`sem_regra` e `regras` são obrigatórios")
	}
	base, err := cel.NewEnv(append(funcoesSemantica(), cel.CrossTypeNumericComparisons(true))...)
	if err != nil {
		return nil, err
	}
	m := &Motor{semRegra: arq.SemRegra, variaveis: arq.Variaveis, esquema: &esquema{}}
	m.esquema.registrar([]string{"id"})
	vistos := map[string]bool{arq.SemRegra: true}
	for _, r := range arq.Regras {
		if r.Codigo == "" || vistos[r.Codigo] {
			return nil, fmt.Errorf("código vazio ou repetido: %q", r.Codigo)
		}
		vistos[r.Codigo] = true
		prg, err := compilar(base, r.Condicao, m.esquema)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", r.Codigo, err)
		}
		m.regras = append(m.regras, regra{r.Codigo, prg})
	}
	return m, nil
}

// compilar declara como `dyn` cada identificador que a expressão usa (os campos vêm do JSON,
// não de um esquema fixo) e faz a checagem de tipos do CEL. Função inexistente, aridade errada
// ou expressão que não devolve booleano falham aqui, na carga, e não em silêncio por linha.
func compilar(base *cel.Env, expr string, esq *esquema) (cel.Program, error) {
	analisada, iss := base.Parse(expr)
	if iss.Err() != nil {
		return nil, iss.Err()
	}
	esq.registrarExpressao(analisada.NativeRep().Expr())
	nomes := map[string]bool{}
	ast.PreOrderVisit(analisada.NativeRep().Expr(), ast.NewExprVisitor(func(e ast.Expr) {
		if e.Kind() == ast.IdentKind {
			nomes[e.AsIdent()] = true
		}
	}))
	var vars []cel.EnvOption
	for n := range nomes {
		vars = append(vars, cel.Variable(n, cel.DynType))
	}
	env, err := base.Extend(vars...)
	if err != nil {
		return nil, err
	}
	checada, iss := env.Check(analisada)
	if iss.Err() != nil {
		return nil, iss.Err()
	}
	if checada.OutputType() != cel.BoolType {
		return nil, fmt.Errorf("a condição precisa devolver booleano, devolve %s", checada.OutputType())
	}
	return env.Program(checada, cel.EvalOptions(cel.OptOptimize))
}

// Ler decodifica a linha guardando só os campos citados pelas regras; false se a linha não
// for um objeto JSON válido. O documento aponta para os bytes da linha: não altere a linha
// enquanto ele estiver em uso.
func (m *Motor) Ler(linha []byte) (map[string]any, bool) {
	return m.esquema.ler(linha)
}

// Classificar devolve o código da primeira regra verdadeira. Erro de avaliação = regra falsa.
func (m *Motor) Classificar(doc map[string]any) string {
	doc["variaveis"] = m.variaveis
	ativacao, _ := interpreter.NewActivation(doc) // uma por linha, não uma por regra
	for _, r := range m.regras {
		if v, _, err := r.programa.Eval(ativacao); err == nil && v == types.True {
			return r.codigo
		}
	}
	return m.semRegra
}

func funcoesSemantica() []cel.EnvOption {
	tipo := func(nome string, saida *cel.Type, ok func(ref.Val) bool, conv func(ref.Val) ref.Val) cel.EnvOption {
		return cel.Function(nome, cel.Overload(nome+"_dyn", []*cel.Type{cel.DynType}, saida,
			cel.UnaryBinding(func(v ref.Val) ref.Val {
				if !ok(v) {
					return types.NewErr("%s: valor não é do tipo esperado", nome)
				}
				return conv(v)
			})))
	}
	igual := func(v ref.Val) ref.Val { return v }
	return []cel.EnvOption{
		// Texto: só string JSON. Número: só número JSON (booleano não é número).
		tipo("texto", cel.StringType, func(v ref.Val) bool { return v.Type() == types.StringType }, igual),
		tipo("numero", cel.DoubleType, func(v ref.Val) bool { return v.Type() == types.DoubleType || v.Type() == types.IntType },
			func(v ref.Val) ref.Val { return v.ConvertToType(types.DoubleType) }),
		tipo("booleano", cel.BoolType, func(v ref.Val) bool { return v.Type() == types.BoolType }, igual),
		// Data: só texto AAAA-MM-DD existente no calendário; vira inteiro AAAAMMDD (mantém a ordem).
		tipo("data", cel.IntType, func(v ref.Val) bool {
			s, e := v.Value().(string)
			_, valida := dataAAAAMMDD(s)
			return e && valida
		}, func(v ref.Val) ref.Val {
			n, _ := dataAAAAMMDD(v.Value().(string))
			return types.Int(n)
		}),
		// "Está vazio": ausente, nulo ou "". Não estrita: recebe o erro de campo ausente
		// (ou de pai que não é objeto) em vez de propagá-lo.
		cel.Function("vazio", cel.Overload("vazio_dyn", []*cel.Type{cel.DynType}, cel.BoolType,
			cel.OverloadIsNonStrict(),
			cel.UnaryBinding(func(v ref.Val) ref.Val {
				if types.IsUnknownOrError(v) || v.Type() == types.NullType {
					return types.True
				}
				s, ok := v.Value().(string)
				return types.Bool(ok && s == "")
			}))),
	}
}

func dataAAAAMMDD(s string) (int64, bool) {
	if len(s) != 10 || s[4] != '-' || s[7] != '-' {
		return 0, false
	}
	num := func(b string) (int64, bool) {
		var n int64
		for i := 0; i < len(b); i++ {
			if b[i] < '0' || b[i] > '9' {
				return 0, false
			}
			n = n*10 + int64(b[i]-'0')
		}
		return n, true
	}
	ano, ok1 := num(s[0:4])
	mes, ok2 := num(s[5:7])
	dia, ok3 := num(s[8:10])
	if !ok1 || !ok2 || !ok3 || ano == 0 || mes < 1 || mes > 12 || dia < 1 {
		return 0, false
	}
	dias := [...]int64{31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31}[mes-1]
	if mes == 2 && ((ano%4 == 0 && ano%100 != 0) || ano%400 == 0) {
		dias = 29
	}
	if dia > dias {
		return 0, false
	}
	return ano*10000 + mes*100 + dia, true
}
