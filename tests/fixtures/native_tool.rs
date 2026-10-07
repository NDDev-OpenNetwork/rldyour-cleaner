// Compiled only by integration tests. All deletions are synthetic fixtures.
use std::{env, fs, io::{self, Write}, path::PathBuf, time::Duration};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|s| s=="loud") {
        let data = vec![b'x'; 256*1024];
        io::stdout().write_all(&data).unwrap();
        io::stderr().write_all(&data).unwrap(); return;
    }
    if args.first().is_some_and(|s|s=="heartbeat") {
        let path=PathBuf::from(&args[1]);let mut n=0;
        loop {fs::write(&path,n.to_string()).unwrap();n+=1;std::thread::sleep(Duration::from_millis(10));}
    }
    if args.first().is_some_and(|s|s=="descendants"||s=="exit-with-descendant") {
        let _child=std::process::Command::new(env::current_exe().unwrap()).arg("heartbeat").arg(&args[1]).spawn().unwrap();
        for _ in 0..200 {if PathBuf::from(&args[1]).exists(){break;}std::thread::sleep(Duration::from_millis(5));}
        println!("spawned synthetic descendant");
        if args[0]=="descendants" {std::thread::sleep(Duration::from_secs(30));}return;
    }
    if args.first().is_some_and(|s| s=="sleep") { std::thread::sleep(Duration::from_secs(30));return; }
    if args==["--version"] { println!("uv {}",env::var("FIXTURE_UV_VERSION").unwrap_or_else(|_|"0.12.17".into())); return; }
    if args.get(0).is_some_and(|s|s=="cache") && args.get(1).is_some_and(|s|s=="dir") {
        if env::var_os("FIXTURE_PATH_TRUNCATION").is_some() {
            print!("{}{}", env::var("FIXTURE_CACHE").unwrap(), " ".repeat(20 * 1024)); return;
        }
        println!("{}",env::var("FIXTURE_CACHE").unwrap()); return;
    }
    assert_eq!(&args[..2],["cache","prune"]);
    assert!(!args.iter().any(|s|s=="--force"||s=="--ci"||s=="clean"));
    assert_eq!(env::var("UV_LOCK_TIMEOUT").unwrap(),"5");
    let log = PathBuf::from(env::var("FIXTURE_LOG").unwrap());
    fs::write(log,args.join("\n")).unwrap();
    let cache = PathBuf::from(env::var("FIXTURE_CACHE").unwrap());
    let at=args.iter().position(|s|s=="--cache-dir").unwrap();
    assert_eq!(PathBuf::from(&args[at+1]),cache);
    if cache.join("busy.lock").exists() { eprintln!("cache is busy");std::process::exit(1); }
    let _=fs::remove_file(cache.join("unused"));
    eprintln!("Removed 1 synthetic dangling entry");
}
