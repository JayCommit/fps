//! Curated game addons (mod loaders, plugins, resources) installable from the panel.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveKind {
    Zip,
    TarGz,
    File,
}

impl ArchiveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarGz => "tar_gz",
            Self::File => "file",
        }
    }

    pub fn infer(url: &str) -> Self {
        let lower = url.to_ascii_lowercase();
        let path = lower.split('?').next().unwrap_or(&lower);
        if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
            Self::TarGz
        } else if path.ends_with(".zip") {
            Self::Zip
        } else {
            Self::File
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AddonSource {
    Url {
        url: String,
    },
    GithubRelease {
        owner: String,
        repo: String,
        asset_glob: String,
    },
    GithubArchive {
        owner: String,
        repo: String,
        #[serde(default = "default_branch")]
        branch: String,
    },
    IndexHref {
        index_url: String,
        asset_glob: String,
    },
}

fn default_branch() -> String {
    "main".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilePatch {
    EnsureLine {
        path: String,
        #[serde(default)]
        after_contains: Option<String>,
        line: String,
    },
    RemoveLine {
        path: String,
        contains: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddonSpec {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// `loader`, `framework`, `plugin`, or `resource`.
    pub category: String,
    pub games: Vec<String>,
    /// When non-empty, only these template slugs may install the addon.
    #[serde(default)]
    pub template_slugs: Vec<String>,
    pub version_label: String,
    pub source: AddonSource,
    pub archive: ArchiveKind,
    /// Path relative to the server volume. Empty means volume root.
    pub dest_path: String,
    #[serde(default)]
    pub strip_components: u32,
    pub tracked_paths: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub restart_required: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub post_install: Vec<FilePatch>,
}

impl AddonSpec {
    pub fn matches_template(&self, slug: &str, game: &str) -> bool {
        if !self.template_slugs.is_empty() {
            return self.template_slugs.iter().any(|s| s == slug);
        }
        self.games.iter().any(|g| g == game)
    }

    pub fn uninstall_patches(&self) -> Vec<FilePatch> {
        self.post_install
            .iter()
            .filter_map(|patch| match patch {
                FilePatch::EnsureLine { path, line, .. } => Some(FilePatch::RemoveLine {
                    path: path.clone(),
                    contains: line.trim().to_string(),
                }),
                FilePatch::RemoveLine { .. } => None,
            })
            .collect()
    }
}

/// Returns true when `name` matches a `*` glob (substring wildcards).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    if !pattern.contains('*') {
        return name == pattern;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = name;
    if let Some(prefix) = parts.first() {
        if !prefix.is_empty() {
            let Some(stripped) = rest.strip_prefix(prefix) else {
                return false;
            };
            rest = stripped;
        }
    }
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            if i + 1 == parts.len() {
                return true;
            }
            continue;
        }
        if i + 1 == parts.len() {
            return rest.ends_with(part);
        }
        if let Some(idx) = rest.find(part) {
            rest = &rest[idx + part.len()..];
        } else {
            return false;
        }
    }
    true
}

/// Prefer linux builds, then the highest MetaMod git number, then shorter names.
pub fn asset_rank(name: &str) -> (i32, i64, i32) {
    let lower = name.to_ascii_lowercase();
    let linux = if lower.contains("linux") || lower.contains("unix") {
        2
    } else if lower.contains("win") {
        0
    } else {
        1
    };
    let git = git_build_number(name).unwrap_or(0);
    let shortness = -(name.len() as i32);
    (linux, git, shortness)
}

fn git_build_number(name: &str) -> Option<i64> {
    let bytes = name.as_bytes();
    let needle = b"git";
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i..].len() >= 3 && bytes[i..i + 3].eq_ignore_ascii_case(needle) {
            let mut j = i + 3;
            let start = j;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > start {
                if let Ok(n) = name[start..j].parse::<i64>() {
                    return Some(n);
                }
            }
        }
        i += 1;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn spec(
    slug: &str,
    name: &str,
    description: &str,
    category: &str,
    games: &[&str],
    template_slugs: &[&str],
    source: AddonSource,
    archive: ArchiveKind,
    dest_path: &str,
    strip_components: u32,
    tracked_paths: &[&str],
    depends_on: &[&str],
    restart_required: bool,
    notes: &str,
    homepage: Option<&str>,
    post_install: Vec<FilePatch>,
) -> AddonSpec {
    AddonSpec {
        slug: slug.into(),
        name: name.into(),
        description: description.into(),
        category: category.into(),
        games: games.iter().map(|s| (*s).to_string()).collect(),
        template_slugs: template_slugs.iter().map(|s| (*s).to_string()).collect(),
        version_label: "latest".into(),
        source,
        archive,
        dest_path: dest_path.into(),
        strip_components,
        tracked_paths: tracked_paths.iter().map(|s| (*s).to_string()).collect(),
        depends_on: depends_on.iter().map(|s| (*s).to_string()).collect(),
        restart_required,
        notes: notes.into(),
        homepage: homepage.map(str::to_string),
        post_install,
    }
}

fn github(owner: &str, repo: &str, asset_glob: &str) -> AddonSource {
    AddonSource::GithubRelease {
        owner: owner.into(),
        repo: repo.into(),
        asset_glob: asset_glob.into(),
    }
}

fn github_archive(owner: &str, repo: &str, branch: &str) -> AddonSource {
    AddonSource::GithubArchive {
        owner: owner.into(),
        repo: repo.into(),
        branch: branch.into(),
    }
}

fn url(url: &str) -> AddonSource {
    AddonSource::Url { url: url.into() }
}

fn index_href(index_url: &str, asset_glob: &str) -> AddonSource {
    AddonSource::IndexHref {
        index_url: index_url.into(),
        asset_glob: asset_glob.into(),
    }
}

fn cs2_gameinfo_patch(line: &str) -> FilePatch {
    FilePatch::EnsureLine {
        path: "game/csgo/gameinfo.gi".into(),
        after_contains: Some("Game_LowViolence".into()),
        line: line.into(),
    }
}

/// All addons offered by a fresh control plane.
pub fn seeded() -> Vec<AddonSpec> {
    vec![
        spec(
            "cs2-metamod",
            "MetaMod:Source",
            "CS2 plugin loader. Required by CounterStrikeSharp and most server plugins.",
            "loader",
            &["cs2"],
            &[],
            index_href(
                "https://mms.alliedmods.net/mmsdrop/2.0/",
                "mmsource-2.0.0-git*-linux.tar.gz",
            ),
            ArchiveKind::TarGz,
            "game/csgo",
            0,
            &["game/csgo/addons/metamod", "game/csgo/addons/metamod.vdf"],
            &[],
            true,
            "Patches gameinfo.gi so CS2 loads MetaMod. Restart after install. Start the server once first if gameinfo.gi is not on disk yet.",
            Some("https://www.metamodsource.net/"),
            vec![cs2_gameinfo_patch("\t\t\tGame\tcsgo/addons/metamod")],
        ),
        spec(
            "cs2-counterstrikesharp",
            "CounterStrikeSharp",
            ".NET scripting runtime for CS2 plugins (with Linux runtime bundled).",
            "framework",
            &["cs2"],
            &[],
            github(
                "roflmuffin",
                "CounterStrikeSharp",
                "counterstrikesharp-with-runtime-linux-*.zip",
            ),
            ArchiveKind::Zip,
            "game/csgo",
            0,
            &[
                "game/csgo/addons/counterstrikesharp",
                "game/csgo/addons/metamod/counterstrikesharp.vdf",
            ],
            &["cs2-metamod"],
            true,
            "Installs MetaMod first when it is missing. Drop C# plugins into addons/counterstrikesharp/plugins.",
            Some("https://docs.cssharp.dev/"),
            vec![],
        ),
        spec(
            "cs2-swiftlys2",
            "SwiftlyS2",
            "Standalone CS2 scripting framework that does not need MetaMod.",
            "framework",
            &["cs2"],
            &[],
            github("swiftly-solution", "swiftlys2", "*with-runtime*linux*.zip"),
            ArchiveKind::Zip,
            "game/csgo",
            0,
            &["game/csgo/addons/swiftlys2"],
            &[],
            true,
            "Do not mix with MetaMod/CounterStrikeSharp on the same server unless you know both loaders are compatible.",
            Some("https://github.com/swiftly-solution/swiftlys2"),
            vec![cs2_gameinfo_patch("\t\t\tGame\tcsgo/addons/swiftlys2")],
        ),
        spec(
            "rust-oxide",
            "Oxide / uMod",
            "Rust plugin framework (uMod). Adds the oxide/ directory and plugin host.",
            "framework",
            &["rust"],
            &[],
            github("OxideMod", "Oxide.Rust", "*linux*.zip"),
            ArchiveKind::Zip,
            "",
            0,
            &["oxide"],
            &[],
            true,
            "Uninstall removes the oxide folder. Some managed DLLs Oxide overwrites are left in place.",
            Some("https://umod.org/games/rust"),
            vec![],
        ),
        spec(
            "mc-luckperms",
            "LuckPerms",
            "Permissions plugin for Paper. Config lands in plugins/LuckPerms.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            url("https://download.luckperms.net/latest/bukkit"),
            ArchiveKind::File,
            "plugins/LuckPerms.jar",
            0,
            &["plugins/LuckPerms.jar", "plugins/LuckPerms"],
            &[],
            true,
            "Paper only. Restart after install so the plugin loads.",
            Some("https://luckperms.net/"),
            vec![],
        ),
        spec(
            "mc-vault",
            "Vault",
            "Economy and permissions bridge used by many Paper plugins.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            github("MilkBowl", "Vault", "Vault*.jar"),
            ArchiveKind::File,
            "plugins/Vault.jar",
            0,
            &["plugins/Vault.jar", "plugins/Vault"],
            &[],
            true,
            "Paper only.",
            Some("https://github.com/MilkBowl/Vault"),
            vec![],
        ),
        spec(
            "mc-essentialsx",
            "EssentialsX",
            "Core commands, homes, kits, and spawn for survival Paper servers.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            github("EssentialsX", "Essentials", "EssentialsX-*.jar"),
            ArchiveKind::File,
            "plugins/EssentialsX.jar",
            0,
            &["plugins/EssentialsX.jar", "plugins/Essentials"],
            &["mc-vault"],
            true,
            "Installs Vault first. Extra modules (Chat, Spawn, …) are not bundled.",
            Some("https://essentialsx.net/"),
            vec![],
        ),
        spec(
            "mc-placeholderapi",
            "PlaceholderAPI",
            "Placeholder expansion host used by scoreboards, chat, and GUIs.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            github("PlaceholderAPI", "PlaceholderAPI", "PlaceholderAPI-*.jar"),
            ArchiveKind::File,
            "plugins/PlaceholderAPI.jar",
            0,
            &["plugins/PlaceholderAPI.jar", "plugins/PlaceholderAPI"],
            &[],
            true,
            "Paper only.",
            Some("https://placeholderapi.com/"),
            vec![],
        ),
        spec(
            "mc-spark",
            "spark",
            "Profiler and TPS monitor for Paper.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            github("lucko", "spark", "spark-*-bukkit.jar"),
            ArchiveKind::File,
            "plugins/spark.jar",
            0,
            &["plugins/spark.jar", "plugins/spark"],
            &[],
            true,
            "Paper only.",
            Some("https://spark.lucko.me/"),
            vec![],
        ),
        spec(
            "mc-worldedit",
            "WorldEdit",
            "In-game map editor for Paper.",
            "plugin",
            &["minecraft"],
            &["minecraft-paper"],
            github("EngineHub", "WorldEdit", "worldedit-bukkit-*.jar"),
            ArchiveKind::File,
            "plugins/WorldEdit.jar",
            0,
            &["plugins/WorldEdit.jar", "plugins/WorldEdit"],
            &[],
            true,
            "Paper only.",
            Some("https://worldedit.enginehub.org/"),
            vec![],
        ),
        spec(
            "fivem-oxmysql",
            "oxmysql",
            "MySQL library used by ox_ and many FiveM frameworks.",
            "resource",
            &["fivem"],
            &[],
            github_archive("overextended", "oxmysql", "main"),
            ArchiveKind::Zip,
            "resources/oxmysql",
            1,
            &["resources/oxmysql"],
            &[],
            true,
            "Add `ensure oxmysql` to server.cfg (or txAdmin) after install.",
            Some("https://github.com/overextended/oxmysql"),
            vec![],
        ),
        spec(
            "fivem-ox-lib",
            "ox_lib",
            "Shared UI, callbacks, and locales used by Overextended resources.",
            "resource",
            &["fivem"],
            &[],
            github_archive("overextended", "ox_lib", "main"),
            ArchiveKind::Zip,
            "resources/ox_lib",
            1,
            &["resources/ox_lib"],
            &[],
            true,
            "Add `ensure ox_lib` to server.cfg before resources that depend on it.",
            Some("https://overextended.dev/ox_lib"),
            vec![],
        ),
        spec(
            "fivem-ox-core",
            "ox_core",
            "Overextended player/vehicle framework.",
            "framework",
            &["fivem"],
            &[],
            github_archive("overextended", "ox_core", "main"),
            ArchiveKind::Zip,
            "resources/ox_core",
            1,
            &["resources/ox_core"],
            &["fivem-oxmysql", "fivem-ox-lib"],
            true,
            "Installs oxmysql and ox_lib first. Ensure those resources in server.cfg before ox_core.",
            Some("https://overextended.dev/ox_core"),
            vec![],
        ),
        spec(
            "fivem-qb-core",
            "qb-core",
            "QBCore framework resource.",
            "framework",
            &["fivem"],
            &[],
            github_archive("qbcore-framework", "qb-core", "main"),
            ArchiveKind::Zip,
            "resources/[qb]/qb-core",
            1,
            &["resources/[qb]/qb-core"],
            &[],
            true,
            "Add `ensure qb-core` (and usually oxmysql) in txAdmin. Other qb-* resources are separate.",
            Some("https://docs.qbcore.org/"),
            vec![],
        ),
        spec(
            "fivem-es-extended",
            "ESX Legacy (es_extended)",
            "ESX core resource from esx_core.",
            "framework",
            &["fivem"],
            &[],
            github_archive("esx-framework", "esx_core", "main"),
            ArchiveKind::Zip,
            "resources/[core]",
            1,
            &["resources/[core]"],
            &["fivem-oxmysql"],
            true,
            "Extracts the esx_core tree (including es_extended). Ensure oxmysql then es_extended in server.cfg.",
            Some("https://docs.esx-framework.org/"),
            vec![],
        ),
        spec(
            "gmod-ulib",
            "ULib",
            "Ulysses library required by ULX admin tools.",
            "framework",
            &["gmod"],
            &[],
            github_archive("TeamUlysses", "ulib", "master"),
            ArchiveKind::Zip,
            "garrysmod/addons/ulib",
            1,
            &["garrysmod/addons/ulib"],
            &[],
            true,
            "Restart the GMod server after install.",
            Some("https://ulyssesmod.net/"),
            vec![],
        ),
        spec(
            "gmod-ulx",
            "ULX",
            "Admin menu and commands for Garry's Mod.",
            "plugin",
            &["gmod"],
            &[],
            github_archive("TeamUlysses", "ulx", "master"),
            ArchiveKind::Zip,
            "garrysmod/addons/ulx",
            1,
            &["garrysmod/addons/ulx"],
            &["gmod-ulib"],
            true,
            "Installs ULib first.",
            Some("https://ulyssesmod.net/"),
            vec![],
        ),
    ]
}

pub fn find(slug: &str) -> Option<AddonSpec> {
    seeded().into_iter().find(|a| a.slug == slug)
}

pub fn for_template(slug: &str, game: &str) -> Vec<AddonSpec> {
    seeded()
        .into_iter()
        .filter(|a| a.matches_template(slug, game))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalogue_slugs_are_unique_and_deps_exist() {
        let seeded = seeded();
        let slugs: BTreeSet<_> = seeded.iter().map(|a| a.slug.as_str()).collect();
        assert_eq!(slugs.len(), seeded.len());
        for addon in &seeded {
            assert!(
                addon
                    .slug
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{}",
                addon.slug
            );
            for dep in &addon.depends_on {
                assert!(
                    slugs.contains(dep.as_str()),
                    "{} missing dep {dep}",
                    addon.slug
                );
            }
            for path in addon
                .tracked_paths
                .iter()
                .chain(std::iter::once(&addon.dest_path))
            {
                assert!(!path.contains(".."), "{} unsafe path {path}", addon.slug);
            }
            assert!(!addon.games.is_empty(), "{}", addon.slug);
        }
        assert!(slugs.contains("cs2-metamod"));
        assert!(slugs.contains("cs2-counterstrikesharp"));
        assert!(slugs.contains("rust-oxide"));
        assert!(slugs.contains("mc-luckperms"));
        assert!(slugs.contains("fivem-ox-lib"));
    }

    #[test]
    fn paper_plugins_do_not_match_bedrock_or_vanilla() {
        assert!(find("mc-luckperms")
            .unwrap()
            .matches_template("minecraft-paper", "minecraft"));
        assert!(!find("mc-luckperms")
            .unwrap()
            .matches_template("minecraft-itzg", "minecraft"));
        assert!(!find("mc-luckperms")
            .unwrap()
            .matches_template("minecraft-bedrock", "minecraft"));
        assert!(find("cs2-metamod").unwrap().matches_template("cs2", "cs2"));
        assert!(for_template("http-echo", "demo").is_empty());
    }

    #[test]
    fn glob_and_rank() {
        assert!(glob_match(
            "counterstrikesharp-with-runtime-linux-*.zip",
            "counterstrikesharp-with-runtime-linux-1.0.371.zip"
        ));
        assert!(!glob_match(
            "counterstrikesharp-with-runtime-linux-*.zip",
            "counterstrikesharp-with-runtime-windows-1.0.371.zip"
        ));
        assert!(glob_match(
            "mmsource-2.0.0-git*-linux.tar.gz",
            "mmsource-2.0.0-git1410-linux.tar.gz"
        ));
        assert!(!glob_match(
            "mmsource-2.0.0-git*-linux.tar.gz",
            "mmsource-latest-linux.tar.gz"
        ));
        assert!(
            asset_rank("mmsource-2.0.0-git1410-linux.tar.gz")
                > asset_rank("mmsource-2.0.0-git999-linux.tar.gz")
        );
        let ess = ["EssentialsX-2.21.2.jar", "EssentialsXChat-2.21.2.jar"];
        let best = ess.iter().max_by_key(|n| asset_rank(n)).unwrap();
        assert_eq!(*best, "EssentialsX-2.21.2.jar");
    }

    #[test]
    fn infers_archive_kind() {
        assert_eq!(
            ArchiveKind::infer("https://example.test/a.tar.gz"),
            ArchiveKind::TarGz
        );
        assert_eq!(
            ArchiveKind::infer("https://example.test/a.zip?x=1"),
            ArchiveKind::Zip
        );
        assert_eq!(
            ArchiveKind::infer("https://download.luckperms.net/latest/bukkit"),
            ArchiveKind::File
        );
    }

    #[test]
    fn uninstall_reverses_ensure_line() {
        let mm = find("cs2-metamod").unwrap();
        let patches = mm.uninstall_patches();
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            FilePatch::RemoveLine { path, contains } => {
                assert_eq!(path, "game/csgo/gameinfo.gi");
                assert!(contains.contains("metamod"));
            }
            _ => panic!("expected remove_line"),
        }
    }
}
