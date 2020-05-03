use std::path::{PathBuf, Path};
use std::process::Command;
use log::*;

fn main() {
	if std::env::var_os("RUST_LOG").is_none() {
		std::env::set_var("RUST_LOG", "minecraft_runner=info,warn,error");
	}
	env_logger::init();

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

	let result = Command::new(&java)
		.args(&["-Xmx6G", "-Xms1G", "-jar", "server.jar", "nogui"])
		.spawn()
		.unwrap()
		.wait();

	match result {
		Ok(status) => info!("Minecraft exited with status: {}", status),
		Err(e) => error!("Minecraft exited with error: {:?}", e),
	}

	#[cfg(windows)]
	sender.send(rivatiker::State::Default).unwrap();
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

	let bundled_jre: PathBuf = [&x86_program_files, "Minecraft Launcher", "runtime", "jre-x64", "bin", JAVA]
		.iter().collect();

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
	use winapi::{
		um::shlobj::*,
		ctypes::c_void,
		shared::guiddef::GUID,
	};

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
