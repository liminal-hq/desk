// Links hello as a module: it starts at `app_start` and has an `app_stop` but no `app_event`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
include!("../link.rs");

fn main() {
    emit_link_arguments("app_start", &["app_stop"]);
}
