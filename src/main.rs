use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

struct Arguments {
    slf_path: String,
    out_path: Option<PathBuf>,
    rom_file: Option<File>,
    cdl_file: Option<File>,
}

impl Arguments {
    pub fn new(slf_path: String) -> Self {
        Self {
            slf_path,
            out_path: None,
            rom_file: None,
            cdl_file: None,
        }
    }
}

fn main() {
    println!("ROM breakdown | ZReC - 2022\n");

    let mut args = env::args();
    let mut arguments = Arguments::new(args.next().unwrap());

    arguments.rom_file = match args.next() {
        Some(rom_path) => match File::open(&rom_path) {
            Ok(f) => {
                arguments.out_path = Some(
                    PathBuf::from(&rom_path)
                        .parent()
                        .unwrap()
                        .join(PathBuf::from(&rom_path).file_stem().unwrap()),
                );

                Some(f)
            }
            Err(e) => return println!("\nError: cannot open rom file [{}]", e),
        },
        None => return print_help(arguments.slf_path),
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-cdl" => {
                if let Some(cdl_path) = args.next() {
                    arguments.cdl_file = match File::open(cdl_path) {
                        Ok(f) => Some(f),
                        Err(e) => return println!("\nError: cannot open cdl file [{}]", e),
                    };
                }
            }

            "-h" | "--help" => return print_help(arguments.slf_path),

            _ => {}
        }
    }

    match match analize_file(arguments.rom_file.as_ref().unwrap()) {
        Ok(f) => f(arguments),
        Err(e) => Err(e),
    } {
        Ok(_) => (),
        Err(e) => return println!("\nError: {}", e),
    }

    println!("\nProgram terminated successfully :)");
}

fn analize_file(mut file: &File) -> io::Result<fn(Arguments) -> io::Result<()>> {
    let header = &mut [0; 0x10];

    match file.read(header) {
        Err(e) => Err(io::Error::new(
            e.kind(),
            format!("Cannot read file's header [{}]", e),
        )),
        _ => Ok(()),
    }?;

    // move cursor back to the begining of the file
    file.seek(io::SeekFrom::Start(0))?;

    match header {
        [0x4e, 0x45, 0x53, 0x1a, ..] => Ok(parse_ines),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Format not supported",
        )),
    }
}

fn parse_ines(arguments: Arguments) -> io::Result<()> {
    let header_buffer = &mut [0; 0x10];

    let (out_path, rom_file) = (
        arguments.out_path.as_ref().unwrap(),
        &mut arguments.rom_file.as_ref().unwrap(),
    );

    rom_file.read(header_buffer)?;

    println!("Found iNES file:\n");

    println!(
        "{}\tCONSTANT",
        String::from_utf8_lossy(&header_buffer[0..4])
    );
    let prg_count = &header_buffer[4];
    println!("{}*16KB\tPRG ROM banks count", prg_count);

    let chr_count = &header_buffer[5];
    println!(
        "{}*8KB\t{}",
        chr_count,
        if chr_count > &0 {
            "CHR ROM banks count"
        } else {
            "CHR RAM only"
        }
    );

    let flag6 = &header_buffer[6];
    let flag7 = &header_buffer[7];
    let mapper = ((flag6 & 0xF0) >> 4) | (flag7 & 0xF0);

    println!(
        "{}\tMAPPER (See https://wiki.nesdev.org/w/index.php?title=INES_Mapper_{})",
        mapper,
        format!("{:03}", mapper)
    );
    println!(
        "b{}\tFLAG6 [{} mirroring{}{}{}]",
        format!("{:04b}", flag6 & 0x0F),
        if flag6 & 0x1 == 0 {
            "horizontal"
        } else {
            "vertical"
        },
        if flag6 & 0x2 == 0 {
            ""
        } else {
            ", persistent memory"
        },
        if flag6 & 0x4 == 0 {
            ""
        } else {
            ", 512-byte trainer"
        },
        if flag6 & 0x8 == 0 {
            ""
        } else {
            ", ignore mirroring control"
        }
    );
    println!(
        "b{}\tFLAG7 [{}{}{}]",
        format!("{:04b}", flag7 & 0x0F),
        if flag7 & 0x1 == 0 {
            ""
        } else {
            "VS Unisystem, "
        },
        if flag7 & 0x1 == 0 {
            ""
        } else {
            "PlayChoice-10, "
        },
        if flag7 & 0xC == 8 {
            panic!("NES 2.0")
        } else {
            "iNES"
        },
    );

    let flag8 = &header_buffer[8];
    let flag9 = &header_buffer[9];

    println!("b{:04b}\tFLAG8 PRG RAM size", flag8 & 0xF);
    println!(
        "b{:01b}\tFLAG9 TV System [{}]",
        flag9 & 0x1,
        if flag9 & 0x1 == 0 { "NTSC" } else { "PAL" }
    );

    println!("--------------------\n");
    println!("Output path will be \"{}\"", out_path.display());
    let answer = &mut [0; 1];
    print!("Do you want to proceed [Y/n]: ");
    io::stdout().flush()?; // stdout must be ready to receive user input
    io::stdin().read(answer)?;

    // Continue?
    match answer[0] as char {
        'y' | 'Y' => (),
        ' '.. => {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted.",
            ))
        }
        _ => (),
    };

    // vv write files vv \\
    if match out_path.metadata() {
        Ok(md) => md.is_dir() == false,
        Err(_) => true,
    } {
        std::fs::create_dir(out_path)?;
    }

    println!();
    print!("Header");

    let mut head_file = File::create(out_path.join("header"))?;
    head_file.write(header_buffer)?;
    println!("\t\twritten");

    if flag6 & 0x4 != 0 {
        print!("Trainer Area");

        let trainer_buffer = &mut [0; 0x200];
        rom_file.read(trainer_buffer)?;
        File::create(out_path.join("trainer"))?.write(trainer_buffer)?;
        println!("\t\twritten");
    }

    let prg_buffer = &mut [0; 0x4000];
    print!("PRG banks");

    for i in 0..*prg_count {
        rom_file.read(prg_buffer)?;
        File::create(out_path.join(format!("prg{}.prg", i)))?.write(prg_buffer)?;
    }
    println!("\twritten");

    let chr_buffer = &mut [0; 0x2000];
    print!("CHR banks");

    for i in 0..*chr_count {
        rom_file.read(chr_buffer)?;
        File::create(out_path.join(format!("chr{}.chr", i)))?.write(chr_buffer)?;
    }
    println!("\twritten");

    if let Some(mut cdl_file) = arguments.cdl_file {
        let cdl_prg_buffer = &mut [0; 0x4000];
        let cdl_chr_buffer = &mut [0; 0x2000];
        print!("CDL files");

        for i in 0..*prg_count {
            cdl_file.read(cdl_prg_buffer)?;
            File::create(out_path.join(format!("prg{}.cdl", i)))?.write(cdl_prg_buffer)?;
        }
        for i in 0..*chr_count {
            cdl_file.read(cdl_chr_buffer)?;
            File::create(out_path.join(format!("chr{}.cdl", i)))?.write(cdl_chr_buffer)?;
        }
        println!("\twritten");
    }

    Ok(())
}

fn print_help(slf_path: String) {
    println!("Usage:");
    println!("\t{} rom_file [cdl_file]\n", slf_path);
    println!("This program breaks down a ROM file into its parts and stores them in individual files.");
    println!("Currently supported file format \".nes\"");
}
