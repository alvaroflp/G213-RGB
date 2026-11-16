# G213 RGB

Controle de iluminação do **Logitech G213 Prodigy e G203** no Linux, feito em Rust.
Uma aplicação local, em português, com design inspirado no
[OpenLogi](https://github.com/AprilNEA/OpenLogi): galeria de dispositivos,
imagem do produto, navegação por abas e ajustes junto à prévia do teclado.

![Logitech G213 Prodigy](assets/devices/g213.png)

## O que você pode fazer

- Personalizar uma cor uniforme ou cada uma das **cinco zonas RGB**.
- Selecionar zonas no diagrama ou nos botões numerados, com cor hexadecimal,
  controles RGB, paleta de cores e cópia de uma cor para todas as zonas.
- Usar efeitos nativos: estático, respiração, ciclo, onda ou desligado.
- Ajustar velocidade e direção da onda, com uma simulação animada na interface.
- Ajustar brilho em estático e respiração. **Ciclo e onda definem suas próprias
  cores e brilho no teclado**; nesses efeitos, a interface só oferece os
  controles que funcionam.
- Criar até 32 perfis locais com nomes próprios; selecionar, excluir e desfazer
  a última exclusão. Quatro presets estão incluídos: Aurora, Foco, Oceano e
  Espectro. Os perfis guardam a iluminação, sem alterar o tema da interface.
- Desfazer a última alteração de iluminação e restaurar a iluminação padrão.
- Escolher tema claro, escuro ou do sistema.
- Preparar configurações com o G213 desconectado e aplicá-las automaticamente
  quando ele se conectar. O aplicativo também detecta desconexões em repouso.
- Ver avisos de conexão, permissão e falha de salvamento na própria interface.

A foto identifica o produto; o diagrama separado simula a iluminação e não
representa uma leitura do estado físico dos LEDs. A edição de cores ou zonas
ativa o efeito estático. O interruptor de iluminação preserva o último efeito.

**Esc** volta para os dispositivos (ou fecha um modal). **Ctrl+Z** desfaz a
última alteração de iluminação, exceto enquanto um campo de texto está em uso.
Os sliders também aceitam as setas do teclado quando recebem foco.

## Macros de teclado e mouse

Na galeria, abra **Macros** e selecione **G213 Prodigy** ou **G203**. Escolha
uma tecla de função (F1–F12) no G213 ou um dos botões **Voltar**, **Avançar** e
**Clique da roda** no G203. Nomeie a macro, adicione atalhos como `Ctrl+Shift+T`
e pausas de 10 a 5000 ms, organize a sequência e clique em **Salvar macro**.
Cada gatilho aceita uma sequência de até 24 etapas. Você pode editar ou remover
a sequência ao selecionar o mesmo gatilho novamente.

Para executar macros, instale a regra opcional e reconecte os dispositivos:

```sh
./install.sh --with-macros
```

A regra concede acesso à sessão local ativa para ler as entradas dos modelos
compatíveis e emitir atalhos via `/dev/uinput`. O app roda como usuário comum e
só monitora os gatilhos configurados quando **Executar macros** está ligado.
A macro é executada no aplicativo em foco ao soltar o gatilho, além da ação
original da tecla ou botão. As macros são salvas localmente e funcionam com a
janela aberta ou com o aplicativo na bandeja. Para restaurar a execução após login, use
**Ajustes → Iniciar com o sistema**.

## Escopo e referência

| Dispositivo USB | Identificador | Controles |
| --- | --- | --- |
| G213 Prodigy | `046d:c336` | Cinco zonas, cor uniforme e efeitos nativos |
| G203 Prodigy | `046d:c084` | Cor estática do logo, brilho e matiz |
| G203 LIGHTSYNC | `046d:c092`, `046d:c09d` | Cor estática uniforme nas três zonas, brilho e matiz |

O aplicativo controla um teclado e um mouse por vez. **Buscar dispositivos**
atualiza a galeria e tenta acessar os modelos compatíveis conectados por USB.
O reconhecimento também ocorre automaticamente ao conectar o cabo.

Use **Iluminação dos dispositivos** na galeria para ligar ou desligar teclado
e mouse juntos. Se apenas um estiver aceso, o primeiro clique acende os dois.
Os controles individuais ficam nas telas do teclado e do mouse.

Em **Sincronizar cores**, escolha **Cor compartilhada** para usar a mesma
cor estática, brilho e matiz nos dois dispositivos. O painel mostra a prévia
e o estado de conexão do G213 e do G203. Clique em **Aplicar** para salvar
e enviar as mudanças. Para recuperar as cores e os efeitos anteriores,
selecione **Cores individuais** e aplique.
O brilho escala a cor até o máximo disponível no hardware. A matiz gira o tom
da cor base e está disponível no G213 em estático e respiração.

A identidade do aplicativo é própria. Não há afiliação com OpenLogi ou Logitech.
Nenhum recurso exige conta, telemetria ou download de imagem em tempo de execução.

## Instalar e executar

Requisitos: Linux com X11 ou Wayland e OpenGL; **Rust 1.95 ou mais recente**,
conforme o requisito do egui 0.36. Instale as ferramentas de compilação C e as
bibliotecas de desenvolvimento do ambiente gráfico da sua distribuição.

A forma mais simples é executar o instalador:

```sh
./install.sh
```

Ele compila o aplicativo, copia o executável para `~/.local/bin`, instala os
ícones e cria o atalho **G213 RGB** no menu de aplicativos. Também instala a
regra udev de acesso ao teclado (o `sudo` é solicitado apenas para essa etapa).

Depois da instalação, abra **G213 RGB** no menu do sistema ou execute:

```sh
g213-rgb
```

Caso `~/.local/bin` não esteja no `PATH`, o instalador mostra a linha necessária
para adicioná-lo. Para instalar para todos os usuários em `/usr/local`:

```sh
./install.sh --system
```

| Opção | Ação |
| --- | --- |
| `./install.sh` | Compila e instala para o usuário atual em `~/.local`. |
| `./install.sh --system` | Instala em `/usr/local` (requer `sudo`). |
| `./install.sh --no-build` | Usa o binário já presente em `target/release`. |
| `./install.sh --no-udev` | Não altera as regras udev. |
| `./install.sh --uninstall` | Remove o executável, o atalho, os ícones e a regra udev. |
| `./install.sh --uninstall --purge` | Também apaga a configuração e os perfis. |

Se o teclado já estava conectado, **desconecte e reconecte-o** após a instalação
para que a nova permissão passe a valer. A regra utiliza `uaccess` para permitir
o acesso pela sessão local do usuário; o aplicativo funciona sem root.

### Executar sem instalar

Para testar direto do código-fonte, sem copiar arquivos para o sistema:

```sh
./run.sh
```

O script compila quando necessário e repassa os argumentos para o aplicativo:

```sh
./run.sh --preview
```

Para conhecer a interface sem acessar o hardware nem modificar sua configuração,
use `--preview`. O modo de prévia começa com a configuração padrão e descarta as
alterações ao fechar. Também é possível compilar manualmente:

```sh
cargo build --locked --release
./target/release/g213-rgb
```

## Restaurar após reiniciar

Abra **Ajustes → Iniciar com o sistema** pelo ícone de engrenagem. O aplicativo
usa a inicialização automática XDG para reaplicar suas preferências ao entrar
na sessão gráfica, inclusive quando detectar um dispositivo conectado depois
do login. Ele inicia na bandeja quando **Manter na bandeja ao fechar** está
ativo e o desktop oferece suporte. Se você já usava a inicialização automática,
desative e ative a opção novamente para atualizar o atalho de login.

## Bandeja do sistema

O ícone do **G213 RGB** aparece no painel de ambientes com suporte a
StatusNotifierItem/AppIndicator, como o Zorin com Zorin AppIndicator habilitado
e o KDE Plasma. Por padrão, o **X** esconde a janela e a iluminação e as macros
continuam em execução. No menu do ícone, use **Abrir G213 RGB** para voltar e
**Sair** para encerrar. Em **Ajustes**, você também pode desativar o comportamento
de manter o aplicativo na bandeja ao fechar. Se o painel não oferecer bandeja,
o **X** encerra o aplicativo normalmente.

## Configuração e perfis

As alterações são salvas automaticamente em
`$XDG_CONFIG_HOME/g213-rgb/config.json` ou `~/.config/g213-rgb/config.json`.
O salvamento usa um arquivo temporário seguido de renomeação. Configurações da
versão anterior continuam compatíveis; perfis são acrescentados ao mesmo JSON.

Para fazer backup ou transferir os perfis, copie esse arquivo com o aplicativo
fechado. Os perfis são salvos no computador, não na memória interna do teclado.
Se houver falha ao salvar, a barra inferior informa o erro e a aba Dispositivo
exibe o detalhe. Mudanças de tema ou criação de perfis não reenviam comandos RGB.

## Linha de comando

| Comando | Ação |
| --- | --- |
| `g213-rgb` | Abre a interface. |
| `g213-rgb --background` | Inicia na bandeja quando estiver disponível e ativada nos ajustes. |
| `g213-rgb --preview` | Explora a interface sem hardware ou salvamento. |
| `g213-rgb --diagnose` | Localiza a interface HID e testa o acesso. |
| `g213-rgb --profiles` | Lista perfis locais e predefinidos. |
| `g213-rgb --profile "Aurora"` | Aplica um perfil e salva a seleção após sucesso. |
| `g213-rgb --apply` | Reaplica a configuração salva aos dispositivos disponíveis. |
| `g213-rgb --test` | Envia cinco cores distintas para testar as zonas. |
| `g213-rgb --off` | Apaga a iluminação sem alterar a configuração salva. |
| `g213-rgb --help` | Mostra a ajuda. |

Use a CLI com a interface gráfica fechada para evitar que dois processos
escrevam no dispositivo ou no mesmo arquivo de configuração. `--profile`
preserva a configuração se o teclado não for encontrado ou a aplicação falhar.
`G213_HIDRAW=/dev/hidrawN` permite indicar manualmente a interface de controle.

## Desenvolvimento e validação

Para contribuir com outro dispositivo, siga o [guia de integração em AGENTS.md](AGENTS.md).

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

Os testes cobrem pacotes HID, desconexão em repouso, leitura de configurações
antigas, perfis, validação hexadecimal (incluindo Unicode), simulação dos efeitos
e fluxos de interação da interface sem hardware. A prévia permite conferir
as telas sem enviar comandos ao teclado. A validação dos efeitos físicos
continua dependendo dos dispositivos conectados e com acesso liberado.

| Arquivo | Responsabilidade |
| --- | --- |
| `src/app.rs` | Navegação, edição de iluminação, estado da interface e modais. |
| `src/app/profiles.rs` | Tela de perfis locais. |
| `src/config.rs` | Configuração, snapshots de iluminação, presets e persistência. |
| `src/device.rs` | Descoberta, protocolo HID e comunicação em segundo plano. |
| `src/product.rs` | Carregamento da imagem incorporada do produto. |
| `src/keyboard.rs` | Simulação interativa das cinco zonas. |
| `src/theme.rs`, `src/widgets.rs` | Paleta, tipografia, ícones e controles. |
| `install.sh` | Compila e instala o aplicativo, o atalho e a regra udev. |
| `run.sh` | Compila e executa sem instalar. |
| `assets/g213-rgb.svg`, `assets/g213-rgb.png` | Ícone do atalho e da janela. |
| `udev/60-g213-rgb.rules` | Regra de acesso ao dispositivo. |

## Protocolo e créditos

O G213 expõe uma interface HID vendor-specific (usage page `0xFF43`, report ID
`0x11`) com relatórios de saída de 20 bytes. A implementação segue o protocolo
registrado no [controlador G213 do OpenRGB](https://github.com/CalcProgrammer1/OpenRGB/tree/master/Controllers/LogitechController/LogitechG213Controller).

A galeria, as abas e o editor com imagem do dispositivo seguem a [interface do OpenLogi](https://openlogi.org/). A implementação usa o código e a identidade visual deste projeto.

Código sob MIT. Inter usa a SIL Open Font License 1.1 (veja
`assets/fonts/LICENSE-Inter.txt`) e Phosphor usa MIT. A fotografia e as marcas
do produto pertencem à Logitech e não fazem parte da licença MIT do código;
consulte [a origem da imagem](assets/devices/README.md).
