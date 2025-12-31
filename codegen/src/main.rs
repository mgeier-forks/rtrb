use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use glob::glob;
use minijinja::{Environment, Value, context, path_loader, value::merge_maps};

fn main() {
    let args = std::env::args();
    #[cfg(feature = "watch")]
    let mut watch = false;
    for arg in args.skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("Generates code for all RingBuffer variants.");
                #[cfg(feature = "watch")]
                println!("Use `--watch` to watch for changes in template files.");
                return;
            }
            #[cfg(feature = "watch")]
            "--watch" => {
                watch = true;
            }
            _ => {
                eprintln!("Unsupported command line argument: {arg}");
                std::process::exit(1);
            }
        }
    }
    let codegen_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let template_dir = codegen_dir.join("templates");
    let parent_dir = codegen_dir.join("..");
    let config_dir = codegen_dir.join("configs");
    let mut contexts = vec![];
    for entry in glob(config_dir.join("*.toml").to_str().unwrap()).unwrap() {
        let config_path = entry.unwrap();
        let config_name = config_path
            .file_stem()
            .unwrap()
            .to_owned()
            .into_string()
            .unwrap();
        let contents = fs::read(&config_path).unwrap();
        let contents = String::from_utf8(contents).unwrap();
        let ctx = toml::from_str(&contents).unwrap();
        contexts.push((config_name, ctx));
    }
    for entry in glob(template_dir.join("**/*.rs").to_str().unwrap()).unwrap() {
        let path = entry.unwrap();
        let path = path.strip_prefix(&template_dir).unwrap();
        render(&parent_dir, path, &contexts);
    }
    #[cfg(feature = "watch")]
    if watch {
        use notify::{
            Config, Event, EventKind::Modify, RecommendedWatcher, RecursiveMode::Recursive,
            Watcher as _,
        };
        use std::collections::HashSet;
        let (mut tx, mut rx) = rtrb::RingBuffer::new(128);
        let mut watcher = RecommendedWatcher::new(
            move |res| match res {
                Ok(Event {
                    kind: Modify(_),
                    paths,
                    ..
                }) => {
                    for path in paths {
                        tx.push(path).expect("queue too small");
                    }
                }
                Ok(_) => {}
                Err(error) => eprintln!("Error from notify: {error:?}"),
            },
            Config::default(),
        )
        .unwrap();
        watcher.watch(&template_dir, Recursive).unwrap();
        println!("Watching template files for changes, press Ctrl-C to cancel.");
        loop {
            // Duplicate paths are removed, order doesn't matter.
            for path in HashSet::<PathBuf>::from_iter(rx.read_chunk(rx.slots()).unwrap()) {
                let path = path.strip_prefix(&template_dir).unwrap();
                if path.extension().and_then(OsStr::to_str) != Some("rs") {
                    continue;
                }
                render(&parent_dir, path, &contexts);
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

fn render(dir: &Path, name: &Path, contexts: &[(String, Value)]) {
    let mut env = Environment::empty();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.set_trim_blocks(true);
    env.set_loader(path_loader(dir.join("codegen/templates")));
    let tmpl = env.get_template(name.to_str().unwrap()).unwrap();
    let mut iter = name.iter().map(OsStr::to_str).map(Option::unwrap);
    let subdir = iter.next().unwrap();
    assert!(["src", "tests"].contains(&subdir));
    let rest = PathBuf::from_iter(iter);
    for (name, ctx) in contexts {
        let ctx = merge_maps([context! { module => format!("rtrb::{name}") }, ctx.clone()]);
        let rendered = tmpl.render(ctx).unwrap();
        let path = dir.join(subdir).join(name).join(&rest);
        fs::write(&path, rendered)
            .unwrap_or_else(|err| panic!("unable to write {:?}: {}", &path, err));
    }
    println!("Rendered {name:?}.");
}
