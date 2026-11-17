//! G213 RGB — controlador de iluminação para o Logitech G213 Prodigy.
//!
//! Run without arguments for the graphical interface, or use one of the
//! diagnostic subcommands (`--diagnose`, `--test`, `--off`).

mod app;
mod autostart;
mod config;
mod device;
mod instance;
mod keyboard;
mod macros;
mod product;
mod theme;
mod tray;
mod widgets;

use app::G213App;

const HELP: &str = "\
G213 RGB — controle de iluminação do Logitech G213 Prodigy

USO:
    g213-rgb               abre a interface gráfica
    g213-rgb --background  inicia na bandeja, quando disponível
    g213-rgb --preview     explora a interface sem acessar o teclado ou salvar
    g213-rgb --profiles    lista perfis locais e predefinidos
    g213-rgb --profile N   aplica um perfil pelo nome e salva a seleção
    g213-rgb --apply       reaplica a configuração salva
    g213-rgb --diagnose    diagnostica USB, macros, bandeja e início automático
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => run_gui(false, false),
        ["--background"] => run_gui(false, true),
        ["--preview"] => run_gui(true, false),
        ["--diagnose" | "diagnose"] => run_diagnose(),
        ["--test" | "test"] => with_hardware(device::run_test),
        ["--off" | "off"] => with_hardware(device::run_off),
        ["--profiles"] => {
            let config = config::Config::load();
            for profile in config::builtin_profiles().iter().chain(&config.profiles) {
                println!("{}\t{}", profile.name, profile.lighting.effect.label());
            }
            0
        }
        ["--profile", name] => with_hardware(|| run_saved(Some(name))),
        ["--apply"] => with_hardware(|| run_saved(None)),
        ["--help" | "-h" | "help"] => {
            print!("{HELP}");
            0
        }
        ["--version" | "-V"] => {
            println!("g213-rgb {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("Argumentos inválidos: {}\n", args.join(" "));
            print!("{HELP}");
            2
        }
    }
}

fn run_diagnose() -> i32 {
    let result = device::run_diagnose();
    macros::diagnose_access();
    println!("\nSessão gráfica");
    match tray::host_available() {
        Ok(true) => println!("  Bandeja: disponível"),
        Ok(false) => println!("  Bandeja: nenhum painel registrado"),
        Err(error) => println!("  Bandeja indisponível: {error}"),
    }
    let autostart = if autostart::enabled() {
        "ativado"
    } else {
        "desativado"
    };
    println!("  Iniciar com o sistema: {autostart}");
    println!("  Configuração: {}", config::config_path().display());
    result
}

fn with_hardware(command: impl FnOnce() -> i32) -> i32 {
    match instance::Instance::claim() {
        Ok(Some(_instance)) => command(),
        Ok(None) => {
            eprintln!(
                "Já existe um G213 RGB controlando os dispositivos. Use Sair na bandeja antes de executar este comando."
            );
            1
        }
        Err(error) => {
            eprintln!("Não foi possível reservar o acesso aos dispositivos: {error}");
            1
        }
    }
}

fn run_saved(profile_name: Option<&str>) -> i32 {
    let mut config = config::Config::load();
    if let Some(name) = profile_name {
        let presets = config::builtin_profiles();
        let Some(profile) = config
            .profiles
            .iter()
            .chain(&presets)
            .find(|p| p.name.to_lowercase() == name.trim().to_lowercase())
        else {
            eprintln!("Perfil não encontrado: {name}. Use --profiles para listar.");
            return 2;
        };
        config.lighting = profile.lighting.clone();
    }
    if profile_name.is_some() && config.sync_colors {
        eprintln!("Desative a sincronização na interface para aplicar um perfil ao teclado.");
        return 2;
    }
    let mut found_any = false;
    let mut failed = false;
    for mouse in [false, true] {
        if mouse && profile_name.is_some() {
            continue;
        }
        let Some(found) = device::discover_matching(mouse) else {
            continue;
        };
        found_any = true;
        let lighting = if mouse {
            config.mouse_lighting()
        } else {
            config.keyboard_lighting()
        };
        match found.open().and_then(|device| {
            found
                .model
                .apply(&device, &device::ApplyState::from(&lighting))
        }) {
            Ok(()) => println!("Iluminação aplicada a {}.", found.model.name()),
            Err(err) => {
                eprintln!(
                    "Não foi possível aplicar a iluminação a {}: {err}",
                    found.model.name()
                );
                failed = true;
            }
        }
    }
    if !found_any {
        eprintln!("Nenhum dispositivo compatível encontrado. A configuração foi preservada.");
        return 1;
    }
    if failed {
        return 1;
    }
    if profile_name.is_some()
        && let Err(err) = config.save()
    {
        eprintln!("Iluminação aplicada, mas não foi possível salvar: {err}");
        return 1;
    }
    0
}

/// Bundled 256×256 icon, used by the window manager and the task bar.
fn app_icon() -> Option<egui::IconData> {
    let pixels = image::load_from_memory(include_bytes!("../assets/g213-rgb.png"))
        .ok()?
        .into_rgba8();
    Some(egui::IconData {
        rgba: pixels.as_raw().clone(),
        width: pixels.width(),
        height: pixels.height(),
    })
}

fn run_gui(preview: bool, background: bool) -> i32 {
    let instance = if preview {
        None
    } else {
        match instance::Instance::claim() {
            Ok(Some(instance)) => Some(instance),
            Ok(None) if background => return 0,
            Ok(None) => {
                return match instance::Instance::reopen() {
                    Ok(()) => 0,
                    Err(error) => {
                        eprintln!("O G213 RGB já está em execução ou está ocupado: {error}");
                        1
                    }
                };
            }
            Err(error) => {
                eprintln!("Não foi possível iniciar o G213 RGB: {error}");
                return 1;
            }
        }
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("G213 RGB")
        .with_app_id("g213-rgb")
        .with_inner_size([1120.0, 760.0])
        .with_min_inner_size([900.0, 620.0])
        .with_resizable(true);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    match eframe::run_native(
        "G213 RGB",
        options,
        Box::new(move |cc| Ok(Box::new(G213App::new(cc, preview, background, instance)?))),
    ) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Não foi possível abrir a interface: {err}");
            1
        }
    }
}
