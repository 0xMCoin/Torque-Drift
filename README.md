# GPU Mine Solana Programs 🖥️⛏️

> Ecossistema completo de mineração simulada na blockchain Solana, transcrito dos contratos Solidity originais.

## 🚀 Quick Start

```bash
# 1. Instalar dependências
./install.sh

# 2. Build dos programas
anchor build

# 3. Executar testes
anchor test

# 4. Deploy local
solana-test-validator &
anchor deploy
```

## 📦 Programas Incluídos

| Programa | Descrição | Funcionalidade Principal |
|----------|-----------|-------------------------|
| **CryptoCoin** | Token fungível SPL | Minting, burning, operadores |
| **GPU NFT** | NFTs de equipamentos | Hash power, royalties, boxes |
| **CryptoCoin Sale** | Venda de tokens | Troca USDT → CryptoCoin |
| **GPU Sale** | Venda de GPUs | Sistema referral, queima de tokens |
| **Reward System** | Sistema de mineração | Recompensas com halving |

## 🔧 Funcionalidades Principais

### 💰 Economia do Token
- **Supply máximo**: 27 milhões de CryptoCoin
- **Preço fixo**: 0.125 USDT por token
- **Halving automático**: Redução de recompensas
- **Queima controlada**: Deflação programada

### 🎮 Mecânicas de Jogo
- **GPU Boxes**: Caixas misteriosas com hash power aleatório
- **Sistema Referral**: Recompensas por indicações
- **Mineração Temporal**: Recompensas baseadas em tempo
- **Royalties**: Sistema automático de royalties

### 🛡️ Segurança
- Controles de acesso (Owner/Operators)
- Proteção contra reentrância
- Funcionalidade pausable
- Limites de supply e transações

## 📋 Arquitetura

```
Usuário → Compra Tokens → Compra GPU → Abre Box → Registra → Mineração → Recompensas
```

### Fluxo Completo:
1. **Compra CryptoCoin** com USDT
2. **Compra GPU Box** com referral
3. **Abertura da Box** revela hash power
4. **Registro para mineração**
5. **Mineração periódica** de recompensas
6. **Halving automático** do sistema

## 🧪 Testes

```bash
# Testes unitários
anchor test

# Testes específicos
anchor test -- --grep "cryptocoin"

# Com logs detalhados
anchor test -- --verbose
```

## 📚 Documentação

- **[📖 Documentação Técnica Detalhada](DOCS.md)** - Arquitetura completa, PDAs, instruções
- **[🏗️ Arquitetura do Sistema](ARCHITECTURE.md)** - Diagramas, fluxos, interações
- **[💻 Exemplos Práticos](EXAMPLES.md)** - Código de exemplo, uso típico
- **[📁 Contratos Originais](../contracts/)** - Referência Solidity
- **🧪 Testes** - Exemplos de uso em `tests/`

## 🎯 Casos de Uso

### Para Usuários
- Comprar tokens através do sale program
- Adquirir GPUs com sistema referral
- Minerar recompensas periodicamente

### Para Desenvolvedores
- Integrar com frontend Solana
- Criar interfaces de usuário
- Implementar funcionalidades adicionais

### Para Auditores
- Verificar segurança dos programas
- Validar lógicas de negócio
- Testar casos extremos

## 🔗 Endpoints Importantes

- **Program ID**: `Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS`
- **Token Decimals**: 18 (CryptoCoin), 0 (NFTs)
- **Max Batch Size**: 10 GPUs por transação

## 🤝 Contribuição

1. Fork o projeto
2. Crie uma branch (`git checkout -b feature/nova-funcionalidade`)
3. Commit suas mudanças (`git commit -am 'Adiciona nova funcionalidade'`)
4. Push para a branch (`git push origin feature/nova-funcionalidade`)
5. Abra um Pull Request

## 📄 Licença

Este projeto mantém a mesma licença dos contratos originais (MIT).

---

**Desenvolvido para Solana** | **Transcrito de Solidity** | **Testado e Auditado**
