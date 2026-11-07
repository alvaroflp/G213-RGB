# G213 RGB

Aplicativo em Rust para controlar a iluminação RGB do teclado **Logitech G213
Prodigy** no Linux. A interface segue o sistema visual do
[OpenLogi](https://github.com/aprilnea/openlogi), com textos em português.

## Recursos

- Galeria de dispositivos com o estado da conexão e prévia da iluminação.
- Iluminação uniforme ou por zona (5 áreas: esquerda, centro, direita,
  setas/home e numpad), com teclado interativo.
- Nove cores predefinidas e editor de cor personalizada (`#RRGGBB`).
- Efeitos nativos do teclado: desligado, estático, respiração, ciclo e onda,
  com velocidade e direção.
- Brilho ajustável (aplicado por software sobre as cores enviadas).
- Tema claro, escuro ou do sistema.
- Alterações aplicadas na hora e salvas automaticamente em
  `~/.config/g213-rgb/config.json`.
- Reconexão automática: o último estado é reaplicado quando o teclado volta.

## Requisitos

- Linux com X11 ou Wayland e OpenGL.
- Rust 1.85 ou mais recente (edição 2024).
- Um Logitech G213 Prodigy conectado por USB (`046d:c336`).

## Compilar e executar

```sh
cargo build --release
./target/release/g213-rgb
```

Na primeira execução o teclado provavelmente estará protegido pelo sistema
(`/dev/hidraw*` pertence a `root`). Instale a regra udev uma vez:

```sh
./install.sh
```

O script pede `sudo`, grava `/etc/udev/rules.d/60-g213-rgb.rules` e recarrega as
regras. Depois **desconecte e reconecte** o teclado. A regra usa `uaccess`, de
modo que apenas a sessão do usuário logado recebe acesso — o dispositivo não é
aberto para todos.

## Linha de comando

| Comando | Descrição |
| --- | --- |
| `g213-rgb` | Abre a interface gráfica. |
| `g213-rgb --diagnose` | Mostra a interface encontrada e testa o acesso. |
| `g213-rgb --test` | Pinta as cinco zonas com cores distintas. |
| `g213-rgb --off` | Desliga a iluminação. |
| `g213-rgb --help` | Mostra a ajuda. |

Use `G213_HIDRAW=/dev/hidrawN` para forçar um dispositivo específico.

## Protocolo

O G213 expõe uma interface HID vendor-specific (usage page `0xFF43`, report ID
`0x11`) e aceita relatórios de saída de 20 bytes:

```
Direct:  11 FF 0C 3A <zona 01..05> 01 RR GG BB 02 ...
Effect:  11 FF 0C 3C 00 <modo> RR GG BB ...
```

Modos: `0` desligado, `1` estático, `2` respiração, `3` ciclo, `4` onda.
O layout foi documentado pelo projeto
[OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) em
`LogitechG213Controller`.

## Estrutura

| Arquivo | Conteúdo |
| --- | --- |
| `src/app.rs` | Telas, cabeçalho, rail e modais. |
| `src/config.rs` | Modelo de configuração, persistência e mapeamento de velocidade. |
| `src/device.rs` | Descoberta do HID, pacotes do protocolo e thread de escrita. |
| `src/theme.rs` | Paleta, tipografia e estilo do egui. |
| `src/widgets.rs` | Controles desenhados à mão (botões, switch, sliders, cartões). |
| `src/keyboard.rs` | Ilustração do teclado com as cinco zonas. |
| `udev/60-g213-rgb.rules` | Regra de acesso sem root. |

## Licença

Código sob MIT. A fonte Inter é distribuída sob a SIL Open Font License 1.1
(consulte `assets/fonts/LICENSE-Inter.txt`); os ícones Phosphor são MIT.
