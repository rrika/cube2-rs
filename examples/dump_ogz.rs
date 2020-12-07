use cube2;

fn dump_ogz(data: &[u8]) {
	cube2::ogz::read_header(data);
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	for arg in &args[1..] {
		match std::fs::read(arg) {
			Ok (data) => dump_ogz(&data),
			Err (err) => eprintln!("failed to open {:?}: {:?}", arg, err)
		}
	}
}
