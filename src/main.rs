// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::io::{self, Write};
fn main() {
    match zyvor_macora::cli::run(std::env::args().skip(1).collect()) {
        Ok(()) => {}
        Err(e) if e == "broken pipe" => {}
        Err(e) => {
            let _ = writeln!(io::stderr(), "macora: {}", zyvor_macora::view::safe(&e));
            std::process::exit(1);
        }
    }
}
