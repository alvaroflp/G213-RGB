//! G213 RGB — controlador de iluminação para o Logitech G213 Prodigy.
//!
//! Run without arguments for the graphical interface, or use one of the
//! diagnostic subcommands (`--diagnose`, `--test`, `--off`).

mod app;
mod config;
mod device;
mod keyboard;
mod theme;
mod widgets;

use app::G213App;

const HELP: &str = "\
G213 RGB — controle de iluminação do Logitech G213 Prodigy

USO:
    g213-rgb               abre a interface gráfica
    g213-rgb --diagnose    mostra o dispositivo encontrado e testa o acesso
    g213-rgb --test        pinta as cinco zonas com cores distintas
    g213-rgb --off         desliga a iluminação
    g213-rgb --help        mostra esta ajuda
    g213-rgb --version     mostra a versão

Variáveis de ambiente:
    G213_HIDRAW            caminho alternativo para o dispositivo (ex.: /dev/hidraw2)
    XDG_CONFIG_HOME        pasta base para a configuração
";

fn main() {
    let code = run();
    if code != 0 {
        std::process::exit(code);
    }
}

fn run() -> i32 {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        None => run_gui(),
        Some("--diagnose") | Some("diagnose") => device::run_diagnose(),
        Some("--test") | Some("test") => device::run_test(),
        Some("--off") | Some("off") => device::run_off(),
        Some("--help") | Some("-h") | Some("help") => {
            print!("{HELP}");
            0
        }
        Some("--version") | Some("-V") => {
            println!("g213-rgb {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some(other) => {
            eprintln!("Opção desconhecida: {other}\n");
            print!("{HELP}");
            2
        }
    }
}

fn run_gui() -> i32 {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("G213 RGB")
            .with_app_id("g213-rgb")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([900.0, 620.0])
            .with_resizable(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    match eframe::run_native(
        "G213 RGB",
        options,
        Box::new(|cc| Ok(Box::new(G213App::new(cc)))),
    ) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Não foi possível abrir a interface: {err}");
            1
        }
    }
}
