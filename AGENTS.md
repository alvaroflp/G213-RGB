# Guia de contribuição para o G213 RGB

Este arquivo orienta agentes de IA e pessoas que contribuem com o repositório.
O G213 RGB é um aplicativo Linux em Rust e egui/eframe. A interface e os textos
para o usuário estão em português. Consulte o [README](README.md) para instalar
o aplicativo, executar a prévia e entender os recursos já disponíveis.

## Onde encontrar cada parte

| Responsabilidade | Arquivo |
| --- | --- |
| Identificação USB, seleção da interface, protocolo e reconexão | [src/device.rs](src/device.rs) |
| Configuração, perfis e estado efetivo da iluminação | [src/config.rs](src/config.rs) |
| Galeria, navegação, ações e estado da interface | [src/app.rs](src/app.rs), [src/app/peripherals.rs](src/app/peripherals.rs) |
| Prévia e aplicação da cor compartilhada | [src/app/sync_ui.rs](src/app/sync_ui.rs) |
| Imagens incorporadas e créditos | [src/product.rs](src/product.rs), [assets/devices/README.md](assets/devices/README.md) |
| Gatilhos e execução de macros | [src/app/macro_editor.rs](src/app/macro_editor.rs), [src/macros.rs](src/macros.rs) |
| Interface de linha de comando, instalação e permissões | [src/main.rs](src/main.rs), [install.sh](install.sh), [udev/](udev/) |

O fluxo atual parte de `Config`, resolve a iluminação efetiva do dispositivo
e converte o resultado em `ApplyState`. A interface envia esse estado ao
`DeviceWorker`. O worker localiza e abre a interface HID, despacha para
`Model::apply` e reaplica a última configuração na reconexão. O transporte HID
tem o nome histórico `G213`, mas também abre a interface usada pelo G203.

## Como integrar outro dispositivo

### 1. Confirme a identificação e as capacidades

- Registre o modelo, transporte, VID:PID e capacidades confirmadas. Para USB no
  Linux, comece por `lsusb` e confira `device/uevent`, `device/report_descriptor`
  e o número da interface em `/sys/class/hidraw`. VID:PID pode aparecer em
  várias interfaces do mesmo dispositivo, inclusive na de entrada padrão.
- Use documentação do fabricante, código de referência com licença compatível
  ou capturas verificadas para confirmar o report ID, o comprimento, os campos,
  a inicialização e a sequência de confirmação ou aplicação, se houver.
  Registre no código ou no PR a origem do protocolo e quais variantes foram
  verificadas. A semelhança do nome comercial não garante compatibilidade.
- Use as integrações atuais como referência: `046d:c336` é o G213 Prodigy
  com cinco zonas. `046d:c084` é o G203 Prodigy. `046d:c092` e `046d:c09d`
  são G203 LIGHTSYNC. Prodigy e LIGHTSYNC têm comandos distintos.

### 2. Implemente a descoberta e o protocolo

1. Adicione ou estenda uma variante de `Model` em `src/device.rs`. Atualize
   `Model::name` e `Model::from_uevent` com os IDs exatos e valide o descritor
   da interface de controle em `Model::matches_descriptor`. Confira também
   os filtros de `discover_matching` para a categoria correspondente.
2. Coloque os pacotes em funções testáveis. Encaminhe a inicialização específica
   do hardware em `Model::initialize` e as escritas em `Model::apply`.
   O G203 ilustra duas variantes com o mesmo tipo de controle, mas relatórios
   diferentes. Implemente ligar, desligar e os efeitos anunciados usando os
   comandos confirmados. Mantenha as operações de I/O no worker.
3. Adicione uma regra restrita ao VID:PID confirmado em
   [60-g213-rgb.rules](udev/60-g213-rgb.rules), mantendo o acesso da sessão local
   via `uaccess`. A GUI roda como usuário comum. `G213_HIDRAW` é uma opção
   de diagnóstico específica do G213 e não identifica outro modelo.
4. Revise a CLI em `src/main.rs`. `--apply` percorre teclado e mouse.
   `--diagnose`, `--test`, `--off` e `--profile` operam no fluxo do teclado.
   Se ampliar esses comandos, documente qual dispositivo recebe a operação.

**Limite atual:** há um worker e um estado individual por categoria, com
`discover_matching(mouse: bool)` escolhendo o primeiro dispositivo compatível.
O código também usa a distinção `Model::G213` versus mouse e filtra a
interface USB `01` do mouse. Se o novo modelo for outro teclado, usar outra
interface ou precisar coexistir com modelos da mesma categoria, revise essa
classificação primeiro. Para suportar múltiplas unidades da mesma categoria,
defina identidade por unidade e modele descoberta, estado, reconexão e seleção
na galeria antes de anunciar suporte simultâneo.

### 3. Conecte configuração, interface e macros

- Reutilize `Lighting` para zonas e efeitos ou `SolidLight` para cor uniforme.
  Preserve a leitura do JSON existente com valores padrão para campos novos.
  O teclado ainda usa `#[serde(flatten)]` para manter o formato antigo.
- Configure apenas controles suportados pelo modelo. Use `Config` e
  `ApplyState` como fonte comum da prévia e da escrita. Ao editar na GUI,
  passe pelo fluxo de `G213App::touch` para atualizar o worker e salvar.
  Preserve as cores individuais ao aplicar a cor compartilhada em
  `src/app/sync_ui.rs` e mantenha o interruptor geral com estado explícito
  para os dispositivos que ele controla.
- Adicione o modelo à galeria com nome e estado de acesso. Reutilize a
  paleta e os controles de `src/theme.rs` e `src/widgets.rs`. Inclua a
  imagem local em `assets/devices/` com origem e condições de uso documentadas,
  e carregue em `src/product.rs`. Use apenas imagens cuja redistribuição seja
  permitida e preserve a identidade visual do G213 RGB.
- Se houver macros, confirme os gatilhos reais em `evdev`. Atualize
  `Peripheral::from_ids`, os gatilhos suportados, o editor e a regra opcional
  [60-g213-rgb-macros.rules](udev/60-g213-rgb-macros.rules). A execução usa
  `uinput`, ocorre ao soltar o gatilho e mantém a ação original. Iluminação
  via HID e macros via `evdev` são integrações distintas.
- Atualize a tabela de compatibilidade do [README](README.md), incluindo
  os IDs e recursos efetivamente implementados.

## Como validar a contribuição

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

Ao mudar as regras, execute `udevadm verify udev/60-g213-rgb.rules
udev/60-g213-rgb-macros.rules` em uma versão do udev com suporte a `verify`.

Para um modelo novo, teste a identificação positiva, a rejeição de interfaces
incorretas e os bytes dos relatórios com exemplos verificados. Se a mudança
alterar a persistência ou a sincronização, cubra a leitura da configuração
antiga e a volta às cores individuais. Confira a interface com
`cargo run --locked -- --preview`, que não acessa o hardware nem persiste
configurações. No dispositivo físico, confirme acesso sem root, ligar e
desligar, aplicar a cor ou efeito e reaplicar após reconectar. Se o dispositivo
não estiver disponível, indique quais testes foram simulados e quais ficaram
pendentes no hardware.
