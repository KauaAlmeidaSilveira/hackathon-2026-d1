# syntax=docker/dockerfile:1
# Solução Go + CEL. Contexto de build = raiz do repositório:
#   docker build -f solucao-go_cel/Dockerfile -t motor-go .
#   docker build -f solucao-go_cel/Dockerfile --target teste .   -> roda os testes
FROM golang:1.24-alpine AS base
WORKDIR /app/solucao-go_cel
COPY solucao-go_cel/go.mod solucao-go_cel/go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod go mod download
COPY solucao-go_cel/*.go ./
COPY solucao-go_cel/regras regras

FROM base AS teste
COPY testes /app/testes
RUN --mount=type=cache,target=/go/pkg/mod --mount=type=cache,target=/root/.cache/go-build \
    go test -count=1 ./...

FROM base AS build
RUN --mount=type=cache,target=/go/pkg/mod --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /motor .

FROM scratch
COPY --from=build /motor /motor
COPY solucao-go_cel/regras /regras
WORKDIR /
ENTRYPOINT ["/motor"]
