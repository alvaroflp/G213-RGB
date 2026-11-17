# Plano de melhoria do G213 RGB

A prioridade é tornar previsível o controle do G213 Prodigy e do G203 no
Linux. As próximas funcionalidades partem de estados claros por dispositivo,
reconexão confiável e uma interface consistente entre teclado e mouse.

## P0 · Confiabilidade e uso diário

- [x] Mostrar por dispositivo se a iluminação aguarda conexão, está sendo
  enviada, foi confirmada pelo USB ou falhou. Permitir repetir o envio
  apenas no dispositivo afetado.
- [x] Manter uma única instância. O atalho do aplicativo deve trazer de volta
  a janela aberta na bandeja. Uma segunda abertura no login deve ser ignorada.
- [x] Ampliar o diagnóstico para teclado e mouse, permissões de USB e
  macros, disponibilidade da bandeja e configuração de início automático.
- [x] Rodar formatação, Clippy, testes e validação do instalador no CI.
  Manter um roteiro manual para verificações que dependem de hardware.

Validação de campo: com um dispositivo desconectado, o outro recebe a cor
normalmente. O dispositivo ausente recebe a última cor ao reconectar. A
confirmação USB é apresentada separadamente da prévia visual dos LEDs.
Abrir o atalho repetidamente não cria outro processo residente.

## P1 · Interface, perfis e macros

- Usar a mesma organização para teclado e mouse: imagem, conexão e ajustes
  por capacidade, com iluminação, macros e diagnóstico acessíveis.
- Conferir navegação por teclado, temas claro e escuro e tamanho mínimo de
  janela. Manter a sincronização como ação conjunta na galeria.
- Permitir duplicar, importar e exportar perfis, incluindo a iluminação de
  ambos os dispositivos e a opção de sincronização. Preservar perfis antigos.
- Permitir capturar um atalho no editor de macros e testar a sequência antes
  de salvar. Tornar visíveis conflitos de gatilho e a ação original que também
  é executada.

Critérios de entrega: um perfil exportado é restaurado em outra instalação;
as funções são encontradas nos dois dispositivos; as macros podem ser testadas
sem gravar a sequência em edição.

## P2 · Mais dispositivos

- Separar o cadastro de modelos e capacidades da identidade das unidades
  físicas e de suas configurações.
- Descobrir e exibir múltiplas unidades da mesma categoria. Reassociar uma
  unidade à configuração correta após reconectar.
- Integrar um modelo por vez a partir de um protocolo e de hardware
  disponíveis para validação. Seguir o [AGENTS.md](../AGENTS.md).

Critério de entrega: dois dispositivos da mesma categoria mantêm controles
e configurações separados. A tabela de compatibilidade informa quais
capacidades foram verificadas em hardware.

## Roteiro manual para uma versão

1. Com o G213 e o G203 conectados, aplicar uma nova cor compartilhada. Conferir
   o resultado por dispositivo e observar os LEDs. Voltar a cores individuais.
2. Desconectar um dos dispositivos, aplicar outra cor e reconectá-lo. Confirmar
   a reaplicação da última cor. Simular um problema de acesso, recuperar a
   permissão e repetir o envio ao dispositivo afetado.
3. Fechar a janela pela bandeja, reabrir pelo menu do sistema e confirmar que
   o mesmo processo permanece ativo. Conferir a opção de sair e o lançamento
   após login, com e sem bandeja disponível.
4. Executar uma macro com permissões habilitadas enquanto a janela está
   oculta. Conferir o comportamento nos ambientes Linux anunciados na versão.
