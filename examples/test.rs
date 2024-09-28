use std::env;
use std::fs::File;
use std::io::Read;

fn main() -> std::io::Result<()> {
    let path = env::args()
        .skip(1)
        .next()
        .unwrap_or("./samples/Sample1.ld".into());
    println!("Reading file: {}", path);

    let mut file = File::open(path)?;
    let mut buff = [0_u8; 23056];
    file.read_exact(&mut buff)?;

    let mut iter = buff.iter();

    let mut file_wr = File::open("./test_write.ld")?;
    let mut new_buff = [0_u8; 23056];
    file_wr.read_exact(&mut new_buff)?;

    let mut iter_new = new_buff.iter();

    let mut index = 0;
    while let (Some(orig), Some(alt)) = (iter.next(), iter_new.next()){
        if orig != alt {
            println!("Diff at {}: {} is {}", index, orig, alt);
        }

        index += 1;
    }

    Ok(())
}
