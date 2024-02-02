use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

pub fn process_shader(shader_path: &Path) -> io::Result<()> {
    let main_shader = File::open(shader_path)?;
    let mut reader = BufReader::new(main_shader);

    let mut line = Default::default();
    while let Ok(bytes_read) = reader.read_line(&mut line) {
        // Check if we have reached EOF
        if bytes_read == 0 {
            break;
        }

        let trimmed_line = line.trim_start();
        // Ignore comment lines
        if trimmed_line.starts_with("//") {
            continue;
        }
    }

    Ok(())
}
