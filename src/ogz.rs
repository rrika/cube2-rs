use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use byteorder::ReadBytesExt;
use byteorder::LittleEndian;
use flate2;

#[derive(Debug)]
enum Engine {
	Sauerbraten,
	Tesseract
}

#[derive(Debug)]
enum Var {
	IVar(u32),
	FVar(f32),
	SVar(String)
}

#[derive(Debug, Default, Clone)]
struct VSlot {
	changed: u32,
	params: Vec<(Vec<u8>, [f32; 4])>,
	scale: f32,
	rotation: u32,
	offset: [u32; 2],
	scroll: [f32; 2],
	layer: u32,
	alphaback: f32,
	alphafront: f32,
	colorscale: [f32; 3]
}

#[allow(unused_variables, non_snake_case)]
fn read_vslot(rdr: &mut std::io::Cursor<Vec<u8>>, vs: &mut VSlot, changed: u32) {

	let (VSLOT_SHPARAM, VSLOT_SCALE, VSLOT_ROTATION, VSLOT_OFFSET,
		VSLOT_SCROLL, VSLOT_LAYER, VSLOT_ALPHA, VSLOT_COLOR, VSLOT_NUM) = (0,1,2,3,4,5,6,7,8);

	vs.changed = changed;
	if vs.changed & (1<<VSLOT_SHPARAM) != 0 {
		let numparams = rdr.read_u16::<LittleEndian>().unwrap();
		vs.params.clear();
		for _i in 0..numparams {
			let nlen = rdr.read_u16::<LittleEndian>().unwrap();

			let mut buf: Vec<u8> = Vec::new();
			buf.resize(nlen as usize, 0u8);
			rdr.read(&mut buf[..]).unwrap();
			let name = buf;

			let mut val: [f32; 4] = [0.0f32; 4];
			val[0] = rdr.read_f32::<LittleEndian>().unwrap();
			val[1] = rdr.read_f32::<LittleEndian>().unwrap();
			val[2] = rdr.read_f32::<LittleEndian>().unwrap();
			val[3] = rdr.read_f32::<LittleEndian>().unwrap();
			vs.params.push((name, val))
		}
	}

	if vs.changed & (1<<VSLOT_SCALE) != 0 {
		vs.scale = rdr.read_f32::<LittleEndian>().unwrap()
	}

	if vs.changed & (1<<VSLOT_ROTATION) != 0 {
		vs.rotation = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_OFFSET) != 0 {
		vs.offset[0] = rdr.read_u32::<LittleEndian>().unwrap();
		vs.offset[1] = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_SCROLL) != 0 {
		vs.scroll[0] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.scroll[1] = rdr.read_f32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_LAYER) != 0 {
		vs.layer = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_ALPHA) != 0 {
		vs.alphafront = rdr.read_f32::<LittleEndian>().unwrap();
		vs.alphaback = rdr.read_f32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_COLOR) != 0 {
		vs.colorscale[0] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.colorscale[1] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.colorscale[2] = rdr.read_f32::<LittleEndian>().unwrap();
	}
}

#[allow(unused_variables)]
fn read_cube(x: i32, y: i32, z: i32, s: i32, cur: &mut std::io::Cursor<Vec<u8>>) {
}

fn read_children(x: i32, y: i32, z: i32, s: i32, cur: &mut std::io::Cursor<Vec<u8>>) {
	read_cube(x,   y,   z,   s, cur);
	read_cube(x+s, y,   z,   s, cur);
	read_cube(x,   y+s, z,   s, cur);
	read_cube(x+s, y+s, z,   s, cur);
	read_cube(x,   y,   z+s, s, cur);
	read_cube(x+s, y,   z+s, s, cur);
	read_cube(x,   y+s, z+s, s, cur);
	read_cube(x+s, y+s, z+s, s, cur);
}

#[allow(unused_variables)]
pub fn read_header(zdata: &[u8]) -> std::io::Result<()> {
	let data = {
		let mut data: Vec<u8> = Vec::new();
		let mut decoder = flate2::read::GzDecoder::new(zdata);
		decoder.read_to_end(&mut data)?;
		data
	};

	let mut rdr = Cursor::new(data);

	let mut magic: [u8; 4] = [0; 4];
	rdr.read(&mut magic[..])?;

	let engine = if magic == *b"OCTA" {
		Engine::Sauerbraten
	} else if magic == *b"TMAP" {
		Engine::Tesseract
	} else {
		panic!();
	};

	let version      = rdr.read_u32::<LittleEndian>().unwrap();
	println!("version {:?}", version);
	let headersize   = rdr.read_u32::<LittleEndian>().unwrap();
	let worldsize    = rdr.read_u32::<LittleEndian>().unwrap();
	let numents      = rdr.read_u32::<LittleEndian>().unwrap();
	let numpvs       = rdr.read_u32::<LittleEndian>().unwrap();
	let numlightmaps = match engine {
		Engine::Sauerbraten => rdr.read_u32::<LittleEndian>().unwrap(),
		Engine::Tesseract => 0
	};
	let blendmap     = rdr.read_u32::<LittleEndian>().unwrap();
	let numvars      = rdr.read_u32::<LittleEndian>().unwrap();
	let numvslots    = if version >= 30 {
		rdr.read_u32::<LittleEndian>().unwrap()
	} else {
		0
	};

	println!("{:?}", engine);

	for i in 0..numvars {
		let t = rdr.read_u8().unwrap();
		let l = rdr.read_u16::<LittleEndian>().unwrap();
		let mut buf = Vec::new();
		buf.resize(l as usize, 0u8);
		rdr.read(&mut buf[..]).unwrap();
		//let name = std::str::from_utf8(&mut buf[..]).unwrap();
		let name = buf;
		let v: Var = match t {
			0 => { Var::IVar(rdr.read_u32::<LittleEndian>().unwrap()) },
			1 => { Var::FVar(rdr.read_f32::<LittleEndian>().unwrap()) },
			2 => {
				let l = rdr.read_u16::<LittleEndian>().unwrap();
				let mut buf = Vec::new();
				buf.resize(l as usize, 0u8);
				rdr.read(&mut buf[..]).unwrap();
				let value = std::str::from_utf8(&mut buf[..]).unwrap();
				Var::SVar(value.to_string())
			},
			_ => panic!("var type {:?}", t)
		};
		println!("#{}: {:?} = {:?}", i, name, v);
	}

	{
		let l = rdr.read_u8().unwrap();
		let mut buf = Vec::new();
		buf.resize(l as usize, 0u8);
		rdr.read(&mut buf[..]).unwrap();
		let name = std::str::from_utf8(&mut buf[..]).unwrap();
		println!("gamemode: {}", name);
		let skip = rdr.read_u8().unwrap();
	}

	let eif       = rdr.read_u16::<LittleEndian>().unwrap();
	let extrasize = rdr.read_u16::<LittleEndian>().unwrap();
	let nummru    = rdr.read_u16::<LittleEndian>().unwrap();
	rdr.seek(SeekFrom::Current(nummru as i64 * 2)).unwrap();

	for i in 0..numents {
		let x = rdr.read_f32::<LittleEndian>().unwrap();
		let y = rdr.read_f32::<LittleEndian>().unwrap();
		let z = rdr.read_f32::<LittleEndian>().unwrap();
		let attr1 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr2 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr3 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr4 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr5 = rdr.read_i16::<LittleEndian>().unwrap();
		let t        = rdr.read_u8().unwrap();
		let reserved = rdr.read_u8().unwrap();

		let entdata = (x, y, z, attr1, attr2, attr3, attr4, attr5, t, reserved);
		println!("{:?}", entdata);
	}

	let mut vslots: Vec<VSlot> = Vec::new();

	while vslots.len() < numvslots as usize {
		let changed = rdr.read_i32::<LittleEndian>().unwrap();
		println!("{}:{}/{}", vslots.len(), changed, numvslots);
		if changed < 0 {
			vslots.resize((vslots.len() as isize - changed as isize) as usize, VSlot::default());
		} else {
			let prev = rdr.read_i32::<LittleEndian>().unwrap();
			let mut vslot: VSlot = VSlot::default();
			read_vslot(&mut rdr, &mut vslot, changed as u32);
			println!("{:?}", vslot);
			vslots.push(vslot);
		}
	}

	let hw = (worldsize >> 1) as i32;
	read_children(0, 0, 0, hw, &mut rdr);

	Ok(())
}
