// Links appmain as an application: it starts at `desk_main` and defines no module entry points.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
include!("../link.rs");

fn main() {
    emit_link_arguments("desk_main", &[]);
}
