use anyhow::Result;
use log::*;
use std::env::{current_exe, set_current_dir};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use structopt::StructOpt;

mod find_jar;
#[cfg(target_feature = "webserver")]
mod webserver;

use crate::find_jar::FindServerJar;

#[cfg(target_feature = "webserver")]
use webserver::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "minecraft_runner", author = "aQaTL")]
struct Opt {
	#[structopt(long, default_value = "1GiB")]
	min: human_size::SpecificSize,
	#[structopt(long, default_value = "16GiB")]
	max: human_size::SpecificSize,
}

fn main() -> Result<()> {
	if std::env::var_os("RUST_LOG").is_none() {
		std::env::set_var("RUST_LOG", "minecraft_runner=info,warn,error");
	}
	env_logger::init();
	set_current_dir(current_exe()?.parent().unwrap())?;
	let current_dir = std::env::current_dir()?;

	let opt: Opt = Opt::from_args();

	let min_jvm_size = opt.min.into::<human_size::Mebibyte>().value().floor() as u64;
	let min_jvm_size = format!("{}M", min_jvm_size);

	let max_jvm_size = opt.max.into::<human_size::Mebibyte>().value().floor() as u64;
	let max_jvm_size = format!("{}M", max_jvm_size);

	info!("Min JVM size: {}", min_jvm_size);
	info!("Max JVM size: {}", max_jvm_size);

	let java = match find_java() {
		Some(v) => v,
		None => panic!("Java not found"),
	};

	info!("Java path: {}", java.display());

	let status = Command::new(&java)
		.args(&["-jar", "AutoIpMinecraft.jar", "server.properties"])
		.status();
	if let Err(e) = status {
		error!("Failed to open AutoIpMinecraft.jar: {:?}", e);
	}

	#[cfg(windows)]
	let sender = rivatiker::start_state_setter(rivatiker::State::NoSystemSleep);

	let server_jar = find_jar::find_server_jar(&current_dir)?;

	let server_jar = match server_jar {
		FindServerJar::ServerJar(path) => path,
		FindServerJar::OneUnknownJar(path) => {
			info!("Trying to launch the server using \"{}\".", path.display());
			path
		}
		FindServerJar::MultipleJars(paths) => {
			let chosen_jar = find_jar::ask_which_jar_to_use(&paths)?;
			if let Err(e) = find_jar::save_jar_preference(&chosen_jar, &current_dir) {
				warn!("Failed to store chosen jar preference: {:?}.", e);
			}
			info!("Using \"{}\" to launch the server.", chosen_jar.display());
			chosen_jar
		}
		FindServerJar::PreferredJar(preferred_jar, _jars) => {
			info!(
				"Using previously chosen jar: \"{}\".",
				preferred_jar.display()
			);
			preferred_jar
		}
		FindServerJar::None => {
			anyhow::bail!(
				"No server jars found laying around in the current directory (\"{}\").",
				current_dir.display()
			);
		}
	};

	let server_jar = server_jar.file_name().and_then(OsStr::to_str).unwrap();
	info!("Stripped the jar path a filename: \"{}\"", server_jar);

	let mut minecraft_process = Command::new(&java)
		.args(&[
			&format!("-Xmx{}", max_jvm_size),
			&format!("-Xms{}", min_jvm_size),
			"-Dsun.rmi.dgc.server.gcInterval=2147483646",
			"-XX:+UseG1GC",
			"-XX:+ParallelRefProcEnabled",
			"-XX:MaxGCPauseMillis=50",
			"-XX:+UnlockExperimentalVMOptions",
			//"-XX:+DisableExplicitGC",
			//"-XX:+AlwaysPreTouch",
			"-XX:G1NewSizePercent=30",
			//"-XX:G1MaxNewSizePercent=40",
			"-XX:G1HeapRegionSize=32M",
			"-XX:G1ReservePercent=20",
			"-XX:G1HeapWastePercent=5",
			"-XX:G1MixedGCCountTarget=4",
			"-XX:InitiatingHeapOccupancyPercent=15",
			"-XX:G1MixedGCLiveThresholdPercent=90",
			"-XX:G1RSetUpdatingPauseTimePercent=5",
			//"-XX:SurvivorRatio=32",
			//"-XX:+PerfDisableSharedMem",
			//"-XX:MaxTenuringThreshold=1",
			"-server",
			"-jar",
			server_jar,
			"nogui",
		])
		.spawn()
		.unwrap();

	#[cfg(target_feature = "webserver")]
	{
		let server_stdin = minecraft_process.stdin.take().unwrap();
		start_web_server(server_stdin, "localhost:8080");
	}

	match minecraft_process.wait() {
		Ok(status) => info!("Minecraft exited with status: {}", status),
		Err(e) => error!("Minecraft exited with error: {:?}", e),
	}

	#[cfg(windows)]
	sender.send(rivatiker::State::Default).unwrap();

	Ok(())
}

#[cfg(not(windows))]
const JAVA: &str = "java";
#[cfg(windows)]
const JAVA: &str = "java.exe";

#[cfg(not(windows))]
fn find_java() -> Option<PathBuf> {
	let java = find_java_in(JAVA.as_ref());
	if java.is_some() {
		return java;
	}

	let path: PathBuf = ["/usr/bin", JAVA].iter().collect();
	find_java_in(&path)
}

#[cfg(windows)]
fn find_java() -> Option<PathBuf> {
	use winapi::um::knownfolders::*;

	let java = find_java_in(JAVA.as_ref());
	if java.is_some() {
		return java;
	}

	let x86_program_files = winutils::get_known_folder(&FOLDERID_ProgramFilesX86)
		.unwrap_or_else(|| String::from(r"C:\Progam Files (x86)"));

	let bundled_jre: PathBuf = [
		&x86_program_files,
		"Minecraft Launcher",
		"runtime",
		"jre-x64",
		"bin",
		JAVA,
	]
	.iter()
	.collect();

	find_java_in(&bundled_jre)
}

fn find_java_in(place: &Path) -> Option<PathBuf> {
	match Command::new(place).arg("-version").output() {
		Ok(output) => {
			if output.status.success() {
				Some(PathBuf::from(place))
			} else {
				debug!("{}", String::from_utf8_lossy(&output.stdout));
				debug!("{}", String::from_utf8_lossy(&output.stderr));
				None
			}
		}
		Err(e) => {
			debug!("{:?}", e);
			None
		}
	}
}

#[cfg(windows)]
mod winutils {
	use winapi::{ctypes::c_void, shared::guiddef::GUID, um::shlobj::*};

	pub fn get_known_folder(folder_id: &GUID) -> Option<String> {
		let mut path: *mut u16 = std::ptr::null_mut();
		let result = unsafe {
			SHGetKnownFolderPath(
				folder_id,
				KF_FLAG_DEFAULT,
				std::ptr::null_mut::<c_void>(),
				&mut path as *mut *mut u16,
			)
		};
		let path = unsafe {
			let mut len = 0usize;
			while *path.add(len) != 0 {
				len += 1;
			}
			let path_str = String::from_utf16_lossy(std::slice::from_raw_parts_mut(path, len));
			winapi::um::combaseapi::CoTaskMemFree(path as *mut c_void);
			path_str
		};
		if result == 0 {
			Some(path)
		} else {
			None
		}
	}
}
