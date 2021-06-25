use anyhow::Result;
use log::*;
use nom::character::complete::{digit1, space0};
use nom::combinator::map_res;
use nom::sequence::preceded;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

pub enum FindServerJar {
	ServerJar(PathBuf),
	OneUnknownJar(PathBuf),
	MultipleJars(Vec<PathBuf>),
	None,
}

pub fn find_server_jar(root: &Path) -> io::Result<FindServerJar> {
	let mut jars: Vec<PathBuf> = std::fs::read_dir(root)?
		.filter_map(|entry| entry.ok())
		.map(|entry| entry.path())
		.filter(|path| path.is_file())
		.filter(|path| path.extension().map(|ext| ext == "jar").unwrap_or_default())
		.collect();

	if jars.is_empty() {
		return Ok(FindServerJar::None);
	}

	if jars.len() == 1 {
		if jars[0]
			.file_name()
			.map(|file_name| file_name == "server.jar")
			.unwrap_or_default()
		{
			Ok(FindServerJar::ServerJar(std::mem::take(&mut jars[0])))
		} else {
			Ok(FindServerJar::OneUnknownJar(std::mem::take(&mut jars[0])))
		}
	} else {
		Ok(FindServerJar::MultipleJars(jars))
	}
}

pub fn ask_which_jar_to_use(jars: &[PathBuf]) -> io::Result<PathBuf> {
	let server_jar: Option<(usize, &PathBuf)> = jars.iter().enumerate().find(|(_idx, path)| {
		path.file_name()
			.map(|file_name| file_name == "server.jar")
			.unwrap_or_default()
	});

	type ServerJarFilterClosure = dyn for<'r, 's> FnMut(&'r (usize, &'s PathBuf)) -> bool;

	let server_jar_filter: Box<ServerJarFilterClosure> = match server_jar {
		Some((server_jar_idx, ref _thing)) => Box::new(move |(idx, _)| *idx != server_jar_idx),
		None => Box::new(move |_| true),
	};

	info!("Multiple jars found: ");

	let mut idx = 1;

	if let Some((_, server_jar)) = server_jar {
		info!("{}. {} (default)", idx, server_jar.display());
		idx += 1;
	}

	for (_, file_name) in jars.iter().enumerate().filter(server_jar_filter) {
		info!("{}. {}", idx, file_name.display());
		idx += 1;
	}

	let stdin = std::io::stdin();
	let idx = loop {
		info!("Choose which one to use [<1,{}>]: ", jars.len());

		let mut line = String::new();
		stdin.read_line(&mut line)?;

		if server_jar.is_some() && line.trim().is_empty() {
			break 0;
		}

		match parse_number_in_range(&line, 1..=jars.len()) {
			Ok(num) => break num - 1,
			Err(e) => error!("{:?}", e),
		}
	};

	Ok(get_jar_from_jars(jars, idx, server_jar.map(|(idx, _)| idx)))
}

fn get_jar_from_jars(jars: &[PathBuf], idx: usize, server_jar_idx: Option<usize>) -> PathBuf {
	let server_jar_idx = match server_jar_idx {
		Some(idx) => idx,
		None => return jars[idx].clone(),
	};

	match idx {
		0 => jars[server_jar_idx].clone(),
		idx if (1..=server_jar_idx).contains(&idx) => jars[idx - 1].clone(),
		idx if ((server_jar_idx + 1)..=usize::MAX).contains(&idx) => jars[idx].clone(),
		_ => unreachable!(),
	}
}

fn parse_number_in_range(number_input: &str, range: RangeInclusive<usize>) -> Result<usize> {
	let parser_result = map_res::<_, _, _, nom::error::Error<&str>, _, _, _>(
		preceded(space0, digit1),
		|num: &str| num.parse::<usize>(),
	)(number_input);

	let (_, number) = match parser_result {
		Ok(v) => v,
		Err(e) => anyhow::bail!("Provided input is not a valid number ({:?})", e),
	};

	if !range.contains(&number) {
		anyhow::bail!("Provided input is not within the required range");
	}

	Ok(number)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn idx_no_server_jar() {
		let jars = vec![
			PathBuf::from("s1.jar"),
			PathBuf::from("s2.jar"),
			PathBuf::from("s3.jar"),
			PathBuf::from("s4.jar"),
		];
		assert_eq!(get_jar_from_jars(&jars, 0, None), PathBuf::from("s1.jar"));
		assert_eq!(get_jar_from_jars(&jars, 1, None), PathBuf::from("s2.jar"));
		assert_eq!(get_jar_from_jars(&jars, 2, None), PathBuf::from("s3.jar"));
		assert_eq!(get_jar_from_jars(&jars, 3, None), PathBuf::from("s4.jar"));
	}

	#[test]
	fn idx_with_server_jar() {
		let jars = vec![
			PathBuf::from("s1.jar"),
			PathBuf::from("s2.jar"),
			PathBuf::from("server.jar"),
			PathBuf::from("s3.jar"),
			PathBuf::from("s4.jar"),
		];
		let server_jar_idx = Some(2);
		assert_eq!(
			get_jar_from_jars(&jars, 0, server_jar_idx),
			PathBuf::from("server.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 1, server_jar_idx),
			PathBuf::from("s1.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 2, server_jar_idx),
			PathBuf::from("s2.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 3, server_jar_idx),
			PathBuf::from("s3.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 4, server_jar_idx),
			PathBuf::from("s4.jar")
		);
	}

	#[test]
	fn idx_with_server_jar_at_the_beginning() {
		let jars = vec![
			PathBuf::from("server.jar"),
			PathBuf::from("s1.jar"),
			PathBuf::from("s2.jar"),
			PathBuf::from("s3.jar"),
			PathBuf::from("s4.jar"),
		];
		let server_jar_idx = Some(0);
		assert_eq!(
			get_jar_from_jars(&jars, 0, server_jar_idx),
			PathBuf::from("server.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 1, server_jar_idx),
			PathBuf::from("s1.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 2, server_jar_idx),
			PathBuf::from("s2.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 3, server_jar_idx),
			PathBuf::from("s3.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 4, server_jar_idx),
			PathBuf::from("s4.jar")
		);
	}

	#[test]
	fn idx_with_server_jar_at_the_end() {
		let jars = vec![
			PathBuf::from("s1.jar"),
			PathBuf::from("s2.jar"),
			PathBuf::from("s3.jar"),
			PathBuf::from("s4.jar"),
			PathBuf::from("server.jar"),
		];
		let server_jar_idx = Some(4);
		assert_eq!(
			get_jar_from_jars(&jars, 0, server_jar_idx),
			PathBuf::from("server.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 1, server_jar_idx),
			PathBuf::from("s1.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 2, server_jar_idx),
			PathBuf::from("s2.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 3, server_jar_idx),
			PathBuf::from("s3.jar")
		);
		assert_eq!(
			get_jar_from_jars(&jars, 4, server_jar_idx),
			PathBuf::from("s4.jar")
		);
	}
}
