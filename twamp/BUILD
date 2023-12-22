load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library")
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")
package(default_visibility = ["//visibility:public"])
# Step 1: Build the binary
rust_binary(
    name = "twamp",
    srcs = glob([
        "src/**/*.rs",
    ]),
    deps = all_crate_deps(
        normal = True,
    ) + ["//network_commons"],
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    edition = "2021",
)


# Step 2: Compress it to layer using pkg_tar
pkg_tar(
    name = "twamp_layer",
    srcs = [":twamp"],
)

load("@rules_oci//oci:defs.bzl", "oci_image")

# Step 3: Build image and add built layer to it
oci_image(
    name = "twamp_image",
    base = "@distroless_cc",
    tars = [":twamp_layer"],
    entrypoint = ["/twamp"],
)