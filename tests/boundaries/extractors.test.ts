import assert from "node:assert/strict";
import { test } from "node:test";

import { specifiersOf } from "../../tooling/boundaries/imports.ts";
import { declaredDependencies } from "../../tooling/boundaries/manifests.ts";

/**
 * An extractor that silently under-reports turns the whole guard into a lie: every rule downstream
 * concludes "nothing forbidden here" from "I did not see it". These test the two parsers directly,
 * rather than only through fixtures, because that is the failure they can have.
 */

const rust = (source: string): string[] => specifiersOf("x.rs", source) ?? [];

test("un groupe use imbriqué est développé jusqu'aux feuilles", () => {
  assert.deepEqual(rust("use tokio::{net::{TcpListener, TcpStream}, fs};"), [
    "tokio/net/TcpListener",
    "tokio/net/TcpStream",
    "tokio/fs",
  ]);
});

test("un alias ne masque pas le chemin importé", () => {
  assert.deepEqual(rust("use bollard::Docker as Runtime;"), ["bollard/Docker"]);
});

test("un import intra-crate n'est pas un import externe", () => {
  assert.deepEqual(rust("use crate::domain::Mission;\nuse super::x;\nuse self::y;"), []);
});

test("un import en commentaire ne compte pas", () => {
  assert.deepEqual(rust("// use bollard::Docker;\n/* use pg::Client; */\nuse serde::Serialize;"), [
    "serde/Serialize",
  ]);
});

test("extern crate et le glob sont vus", () => {
  // Les `use` sont collectés avant les `extern crate` : ordre stable, sans signification.
  assert.deepEqual(rust("extern crate libc;\nuse std::fs::*;"), ["std/fs/*", "libc"]);
});

test("un crate à tiret est reconnu sous ses deux orthographes", () => {
  assert.deepEqual(rust("use tokio_postgres::Client;"), [
    "tokio_postgres/Client",
    "tokio-postgres/Client",
  ]);
});

test("pub use compte comme use", () => {
  assert.deepEqual(rust("pub use sqlx::PgPool;"), ["sqlx/PgPool"]);
});

const cargo = (source: string): string[] => declaredDependencies("Cargo.toml", source) ?? [];

test("les dépendances Cargo sont lues dans toutes leurs formes", () => {
  const manifest = [
    "[package]",
    'name = "locusd"',
    "",
    "[dependencies]",
    'serde = { version = "1" }',
    "bollard = { workspace = true } # commentaire",
    "tokio.workspace = true",
    "",
    "[target.'cfg(unix)'.dependencies]",
    'nix = "0.29"',
    "",
    "[dev-dependencies.insta]",
    'version = "1"',
    "",
    "[build-dependencies]",
    'cc = "1"',
  ].join("\n");
  assert.deepEqual(cargo(manifest), ["serde", "bollard", "tokio", "nix", "insta", "cc"]);
});

test("le nom du paquet lui-même n'est pas une dépendance", () => {
  assert.deepEqual(cargo('[package]\nname = "locusd"\nversion = "0.0.0"'), []);
});

test("un manifeste inconnu n'est pas deviné", () => {
  assert.equal(declaredDependencies("pyproject.toml", "[project]\nname = 'x'"), null);
});
