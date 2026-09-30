//! Full local lifecycle through Tauri's real IPC decoder. The parent starts a
//! dedicated process and ssh-agent with an isolated workspace, never changing HOME.
use crate::commands::{
    agent_policy::*, diagnostics::*, key_import::*, key_insights::*, profiles::*, repositories::*,
};
use crate::commands::{app::*, known_hosts::*, launcher::*, ssh_config::*, ssh_keys::*};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn local_workflow() {
    if std::env::var_os("SSH_GUI_ACCEPTANCE_CHILD").is_none() {
        let workspace = tempfile::Builder::new()
            .prefix("ssh-gui-acceptance-")
            .tempdir_in("/private/tmp")
            .unwrap();
        let socket = workspace.path().join("agent.sock");
        let mut agent = Process(
            Command::new("ssh-agent")
                .env("SSH_ASKPASS", "/usr/bin/false")
                .env("DISPLAY", ":fixture")
                .args(["-D", "-a"])
                .arg(&socket)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let start = Instant::now();
        while !socket.exists() {
            assert!(agent.0.try_wait().unwrap().is_none(), "Test agent exited");
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Test agent did not start"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "acceptance::local_workflow", "--nocapture"])
            .env("SSH_GUI_ACCEPTANCE_CHILD", "1")
            .env("SSH_GUI_WORKSPACE", workspace.path())
            .env("SSH_AUTH_SOCK", &socket)
            .env(
                "SSH_GUI_CAPTURE_TERMINAL",
                workspace.path().join("terminal.txt"),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
        let missing_agent = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "acceptance::local_workflow", "--nocapture"])
            .env("SSH_GUI_ACCEPTANCE_CHILD", "1")
            .env("SSH_GUI_ACCEPTANCE_NO_AGENT", "1")
            .env("SSH_GUI_WORKSPACE", workspace.path())
            .env("SSH_AUTH_SOCK", workspace.path().join("missing.sock"))
            .output()
            .unwrap();
        assert!(
            missing_agent.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&missing_agent.stdout),
            String::from_utf8_lossy(&missing_agent.stderr)
        );
        return;
    }
    let app = tauri::test::mock_builder()
        .invoke_handler(tauri::generate_handler![
            set_repository_roots,
            scan_repositories,
            get_key_usage,
            list_key_metadata,
            save_key_metadata,
            audit_ssh_keys,
            diagnose_ssh,
            inspect_key_import,
            import_ssh_key,
            restore_public_key,
            list_agent_enrollments,
            list_git_profiles,
            save_git_profile,
            delete_git_profile,
            get_workspace,
            get_ssh_config,
            add_host,
            update_host,
            delete_host,
            reorder_hosts,
            list_ssh_keys,
            generate_ssh_key,
            get_public_key,
            delete_ssh_key,
            get_key_fingerprint,
            list_agent_keys,
            add_key_to_agent,
            remove_key_from_agent,
            audit_permissions,
            fix_permissions,
            list_known_hosts,
            delete_known_hosts,
            verify_known_host,
            list_backups,
            restore_backup,
            launch_ssh_connection,
            copy_key_to_server,
            get_app_config,
            save_app_config,
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let call = |command: &str, body: Value| -> Result<Value, String> {
        tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|response| response.deserialize::<Value>().unwrap())
        .map_err(|e| e.to_string())
    };
    let invoke = |command: &str, body: Value| {
        call(command, body).unwrap_or_else(|e| panic!("{command}: {e}"))
    };
    let context = invoke("get_workspace", json!({}));
    assert_eq!(context["isolated"], true);
    let root = Path::new(context["ssh_dir"].as_str().unwrap());
    if std::env::var_os("SSH_GUI_ACCEPTANCE_NO_AGENT").is_some() {
        assert!(call("list_agent_keys", json!({}))
            .unwrap_err()
            .contains("SSH Agent is not available"));
        return;
    }
    assert!(invoke("get_ssh_config", json!({}))
        .as_array()
        .unwrap()
        .is_empty());
    let key = invoke(
        "generate_ssh_key",
        json!({"params": {"algorithm":"Ed25519", "filename":"work", "comment":"local acceptance", "passphrase":null}}),
    );
    let keypath = key["private_path"].as_str().unwrap();
    let public = invoke("get_public_key", json!({"key_path":keypath}));
    assert!(public.as_str().unwrap().starts_with("ssh-ed25519 "));
    assert_eq!(
        invoke("get_key_fingerprint", json!({"key_path":keypath})),
        key["fingerprint"]
    );
    assert_eq!(invoke("list_agent_keys", json!({})), json!([]));
    assert_eq!(
        invoke("add_key_to_agent", json!({"key_path":keypath})),
        true
    );
    assert_eq!(
        invoke("list_agent_keys", json!({})),
        json!([key["fingerprint"]])
    );
    invoke("remove_key_from_agent", json!({"key_path":keypath}));
    assert_eq!(invoke("list_agent_keys", json!({})), json!([]));
    let encrypted = invoke(
        "generate_ssh_key",
        json!({"params": {"algorithm":"Ed25519", "filename":"protected", "comment":"encrypted fixture", "passphrase":"fixture-only-password"}}),
    );
    let encrypted_path = encrypted["private_path"].as_str().unwrap();
    assert_eq!(encrypted["has_passphrase"], true);
    let derived = Command::new("ssh-keygen")
        .args(["-y", "-P", "fixture-only-password", "-f", encrypted_path])
        .output()
        .unwrap();
    assert!(
        derived.status.success(),
        "OpenSSH must decrypt keys produced by the app"
    );
    std::fs::remove_file(format!("{encrypted_path}.pub")).unwrap();
    assert!(invoke("get_public_key", json!({"key_path":encrypted_path}))
        .as_str()
        .unwrap()
        .starts_with("ssh-ed25519 "));
    assert_eq!(
        invoke(
            "add_key_to_agent",
            json!({"key_path":encrypted_path,"options":{"lifetime_seconds":120,"confirm":true}})
        ),
        false
    );
    let terminal_log =
        std::fs::read_to_string(std::env::var_os("SSH_GUI_CAPTURE_TERMINAL").unwrap()).unwrap();
    assert!(terminal_log.contains("SSH_AUTH_SOCK=") && terminal_log.contains("protected"));
    assert!(terminal_log.contains("'-t' '120' '-c'"));
    // Execute the exact command prepared for a terminal with a fixture-only askpass helper.
    let helper = root.join("fixture-askpass.sh");
    std::fs::write(&helper, "#!/bin/sh\nprintf '%s\\n' fixture-only-password\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let enrollment = Command::new("sh")
        .args(["-c", &terminal_log])
        .env("SSH_ASKPASS", &helper)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", ":fixture")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        enrollment.status.success(),
        "{}",
        String::from_utf8_lossy(&enrollment.stderr)
    );
    assert!(invoke("list_agent_keys", json!({}))
        .as_array()
        .unwrap()
        .contains(&encrypted["fingerprint"]));
    invoke("remove_key_from_agent", json!({"key_path":encrypted_path}));
    std::fs::remove_file(helper).unwrap();
    // Nested files are listed by relative path; private and public keys must agree.
    std::fs::create_dir(root.join("team")).unwrap();
    std::fs::copy(keypath, root.join("team/work")).unwrap();
    std::fs::copy(format!("{keypath}.pub"), root.join("team/work.pub")).unwrap();
    assert!(invoke("list_ssh_keys", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["name"] == "team/work"));
    std::fs::write(
        root.join("id_broken"),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nbroken\n",
    )
    .unwrap();
    let keys = invoke("list_ssh_keys", json!({}));
    let broken = keys
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "id_broken")
        .unwrap();
    assert!(broken["has_passphrase"].is_null() && broken["error"].is_string());
    std::fs::copy(encrypted_path, root.join("mismatch")).unwrap();
    std::fs::copy(format!("{keypath}.pub"), root.join("mismatch.pub")).unwrap();
    assert!(
        call("get_public_key", json!({"key_path":root.join("mismatch")}))
            .unwrap_err()
            .contains("does not match")
    );
    // Import an encrypted external key, preserve source bytes, detect duplicates and recover .pub.
    let external = root.parent().unwrap().join("external-key");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "fixture-only-import", "-f"])
        .arg(&external)
        .output()
        .unwrap();
    assert!(generated.status.success());
    std::fs::remove_file(format!("{}.pub", external.display())).unwrap();
    let source_bytes = std::fs::read(&external).unwrap();
    let inspection = invoke("inspect_key_import", json!({"source_path":external}));
    assert_eq!(inspection["public_recovered"], true);
    assert_eq!(inspection["key"]["has_passphrase"], true);
    assert!(inspection["duplicates"].as_array().unwrap().is_empty());
    assert!(call(
        "import_ssh_key",
        json!({"source_path":external,"filename":"imported","revision":"stale"})
    )
    .is_err());
    let imported = invoke(
        "import_ssh_key",
        json!({"source_path":external,"filename":"imported","revision":inspection["revision"]}),
    );
    assert_eq!(
        std::fs::read(imported["private_path"].as_str().unwrap()).unwrap(),
        source_bytes
    );
    assert_eq!(std::fs::read(&external).unwrap(), source_bytes);
    assert_eq!(imported["fingerprint"], inspection["key"]["fingerprint"]);
    assert!(call(
        "import_ssh_key",
        json!({"source_path":external,"filename":"duplicate","revision":inspection["revision"]})
    )
    .is_err());
    assert_eq!(
        invoke("inspect_key_import", json!({"source_path":external}))["duplicates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    std::fs::remove_file(imported["public_path"].as_str().unwrap()).unwrap();
    invoke(
        "restore_public_key",
        json!({"key_path":imported["private_path"],"expected_fingerprint":imported["fingerprint"]}),
    );
    assert!(Path::new(imported["public_path"].as_str().unwrap()).is_file());
    assert!(call(
        "restore_public_key",
        json!({"key_path":imported["private_path"],"expected_fingerprint":imported["fingerprint"]})
    )
    .is_err());
    // Actual agent expiration and confirmation enforcement, independent of the UI clock.
    invoke(
        "add_key_to_agent",
        json!({"key_path":keypath,"options":{"lifetime_seconds":2,"confirm":false}}),
    );
    let mut sign = Command::new("ssh-add");
    sign.arg("-T").arg(format!("{keypath}.pub"));
    assert!(crate::utils::process::run(sign, Duration::from_secs(3))
        .unwrap()
        .status
        .success());
    let start = Instant::now();
    while !invoke("list_agent_keys", json!({}))
        .as_array()
        .unwrap()
        .is_empty()
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Agent lifetime was not enforced"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(call(
        "add_key_to_agent",
        json!({"key_path":keypath,"options":{"lifetime_seconds":0,"confirm":false}})
    )
    .is_err());
    invoke(
        "add_key_to_agent",
        json!({"key_path":keypath,"options":{"lifetime_seconds":60,"confirm":true}}),
    );
    let mut sign = Command::new("ssh-add");
    sign.arg("-T").arg(format!("{keypath}.pub"));
    assert!(
        !crate::utils::process::run(sign, Duration::from_secs(3))
            .unwrap()
            .status
            .success(),
        "Agent must refuse signing when confirmation is denied"
    );
    assert!(invoke("list_agent_enrollments", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["confirm"] == true));
    invoke("remove_key_from_agent", json!({"key_path":keypath}));
    // Git profile lifecycle through IPC, checked by the real OpenSSH parser.
    let original_config =
        "# original defaults\nServerAliveInterval 30\nHost unrelated\n  HostName localhost\n";
    std::fs::write(root.join("config"), original_config).unwrap();
    let empty_profiles = invoke("list_git_profiles", json!({}));
    let profile = json!({"name":"Work", "account":"work-account", "alias":"git-work", "hostname":"github.com", "user":"git", "port":2222, "key_path":keypath});
    let saved = invoke(
        "save_git_profile",
        json!({"profile":profile,"original_alias":null,"revision":empty_profiles["revision"]}),
    );
    assert_eq!(saved["profiles"][0], profile);
    assert!(call(
        "save_git_profile",
        json!({"profile":profile,"original_alias":null,"revision":empty_profiles["revision"]})
    )
    .is_err());
    let effective = Command::new("ssh")
        .args(["-G", "-F"])
        .arg(root.join("config"))
        .arg("git-work")
        .output()
        .unwrap();
    assert!(
        effective.status.success(),
        "{}",
        String::from_utf8_lossy(&effective.stderr)
    );
    let effective = String::from_utf8(effective.stdout).unwrap();
    for expected in [
        "hostname github.com",
        "user git",
        "port 2222",
        "identitiesonly yes",
        "serveraliveinterval 30",
    ] {
        assert!(effective.contains(expected), "missing {expected}");
    }
    assert!(effective.contains(&format!("identityfile {keypath}")));
    let managed = invoke("get_ssh_config", json!({}));
    assert!(managed
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["alias"] == "git-work" && h["read_only"] == true));
    let projects = root.parent().unwrap().join("projects");
    let repository = projects.join("work-repository");
    std::fs::create_dir_all(repository.join(".git")).unwrap();
    std::fs::write(
        repository.join(".git/config"),
        "[remote \"origin\"]\nurl = git@git-work:team/repo.git\n",
    )
    .unwrap();
    invoke("set_repository_roots", json!({"roots":[projects]}));
    let scan = invoke("scan_repositories", json!({}));
    assert_eq!(scan["repositories"], 1);
    assert_eq!(scan["remotes"][0]["key_paths"], json!([keypath]));
    let usage = invoke("get_key_usage", json!({"key_path":keypath}));
    assert_eq!(usage["profiles"], json!(["Work"]));
    assert_eq!(usage["repositories"].as_array().unwrap().len(), 1);
    // Annotations persist by identity across copies and enforce optimistic concurrency.
    let before_metadata = invoke("list_key_metadata", json!({}));
    let annotations = json!({"tags":[" work ","WORK","client"],"purpose":"Git signing fixture","note":"Local notes", "replace_on":"2028-02-29"});
    let request = json!({"key_path":keypath,"expected_fingerprint":key["fingerprint"],"metadata":annotations,"revision":before_metadata["revision"]});
    let saved_metadata = invoke("save_key_metadata", request.clone());
    let fingerprint = key["fingerprint"].as_str().unwrap();
    assert_eq!(
        saved_metadata["entries"][fingerprint]["tags"],
        json!(["work", "client"])
    );
    assert_eq!(invoke("list_key_metadata", json!({})), saved_metadata);
    assert!(call("save_key_metadata", request)
        .unwrap_err()
        .contains("changed"));
    assert!(call("save_key_metadata", json!({"key_path":keypath,"expected_fingerprint":"stale","metadata":annotations,"revision":saved_metadata["revision"]})).unwrap_err().contains("key changed"));
    let copy_saved = invoke(
        "save_key_metadata",
        json!({"key_path":root.join("team/work"),"expected_fingerprint":key["fingerprint"],"metadata":{"tags":["personal"],"purpose":"","note":"","replace_on":null},"revision":saved_metadata["revision"]}),
    );
    assert_eq!(copy_saved["entries"].as_object().unwrap().len(), 1);
    assert_eq!(
        copy_saved["entries"][fingerprint]["tags"],
        json!(["personal"])
    );
    // Read-only audit: file bytes, permissions and modification times must stay intact.
    let files = crate::utils::ssh_dir::regular_files(root).unwrap();
    let before: Vec<_> = files
        .iter()
        .map(|p| {
            (
                p.clone(),
                std::fs::read(p).unwrap(),
                std::fs::metadata(p).unwrap().modified().unwrap(),
                std::fs::metadata(p).unwrap().permissions(),
            )
        })
        .collect();
    let audit = invoke("audit_ssh_keys", json!({}));
    let entries = audit["entries"].as_array().unwrap();
    let work = entries.iter().find(|e| e["key_path"] == keypath).unwrap();
    let codes: Vec<_> = work["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"no-passphrase") && codes.contains(&"copies"));
    assert!(!codes.contains(&"no-local-links"));
    assert_eq!(work["usage"]["profiles"], json!(["Work"]));
    assert_eq!(work["copies"].as_array().unwrap().len(), 2);
    let broken = entries
        .iter()
        .find(|e| e["key_path"] == root.join("id_broken").to_str().unwrap())
        .unwrap();
    assert!(broken["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["code"] == "unreadable"));
    let mismatch = entries
        .iter()
        .find(|e| e["key_path"] == root.join("mismatch").to_str().unwrap())
        .unwrap();
    assert!(mismatch["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["severity"] == "error"));
    for (path, bytes, modified, permissions) in before {
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let after = std::fs::metadata(&path).unwrap();
        assert_eq!(after.modified().unwrap(), modified);
        assert_eq!(after.permissions(), permissions);
    }
    // A failed relationship scan must not claim a complete absence of references.
    let roots_path = root
        .parent()
        .unwrap()
        .join("settings/repository-roots.json");
    let roots_content = std::fs::read(&roots_path).unwrap();
    std::fs::write(&roots_path, "broken json").unwrap();
    let partial = invoke("audit_ssh_keys", json!({}));
    assert!(!partial["warnings"].as_array().unwrap().is_empty());
    for entry in partial["entries"].as_array().unwrap() {
        assert!(entry["usage"].is_null());
        assert!(!entry["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "no-local-links"));
    }
    std::fs::write(&roots_path, roots_content).unwrap();
    let report = invoke("diagnose_ssh", json!({"alias":"git-work","network":false}));
    assert_eq!(report["hostname"], "github.com");
    assert_eq!(report["port"], "2222");
    let overridden = invoke(
        "diagnose_ssh",
        json!({"alias":"another@git-work","network":false,"port":2207}),
    );
    assert_eq!(overridden["port"], "2207");
    assert_eq!(overridden["user"], "another");

    assert!(report["authenticated"].is_null());
    invoke(
        "delete_git_profile",
        json!({"alias":"git-work","revision":saved["revision"]}),
    );
    assert_eq!(
        std::fs::read_to_string(root.join("config")).unwrap(),
        original_config
    );
    // Physical host blocks and included source files round-trip independently.
    std::fs::create_dir(root.join("conf.d")).unwrap();
    std::fs::write(
        root.join("config"),
        "# root\nInclude conf.d/*\nHost root-host\n  HostName localhost\n",
    )
    .unwrap();
    let included = root.join("conf.d/work.conf");
    std::fs::write(
        &included,
        "# included\nHost work-host\n  HostName old # keep\n",
    )
    .unwrap();
    let hosts = invoke("get_ssh_config", json!({}));
    let mut host = hosts
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["alias"] == "work-host")
        .unwrap()
        .clone();
    let stale = host.clone();
    host["hostname"] = json!("new");
    invoke(
        "update_host",
        json!({"original_alias":"work-host", "host":host}),
    );
    assert!(std::fs::read_to_string(&included)
        .unwrap()
        .contains("HostName new # keep"));
    assert!(std::fs::read_to_string(root.join("config"))
        .unwrap()
        .contains("Host root-host"));
    assert!(call(
        "update_host",
        json!({"original_alias":"work-host", "host":stale})
    )
    .unwrap_err()
    .contains("changed since"));
    let after_edit = invoke("get_ssh_config", json!({}));
    assert_eq!(
        after_edit.as_array().unwrap().len(),
        2,
        "Include wildcard must not load backups"
    );
    let backups = invoke("list_backups", json!({}));
    let backup = backups
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["path"] == included.to_string_lossy().as_ref())
        .unwrap()
        .clone();
    invoke(
        "restore_backup",
        json!({"path":backup["path"],"current_revision":backup["current_revision"],"backup_revision":backup["backup_revision"]}),
    );
    assert!(std::fs::read_to_string(&included)
        .unwrap()
        .contains("HostName old # keep"));
    // Backup revision confirmation cannot be replayed.
    assert!(call("restore_backup", json!({"path":backup["path"],"current_revision":backup["current_revision"],"backup_revision":backup["backup_revision"]})).is_err());
    let mut new_host = stale.clone();
    new_host["alias"] = json!("second");
    invoke("add_host", json!({"host":new_host}));
    let hosts = invoke("get_ssh_config", json!({}));
    let first = hosts
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["alias"] == "work-host")
        .unwrap();
    invoke(
        "reorder_hosts",
        json!({"aliases":["second","work-host"],"revision":first["revision"],"source_path":first["source_path"]}),
    );
    let hosts = invoke("get_ssh_config", json!({}));
    let second = hosts
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["alias"] == "second")
        .unwrap();
    invoke(
        "delete_host",
        json!({"alias":second["alias"],"revision":second["revision"],"source_path":second["source_path"],"line_start":second["line_start"]}),
    );
    assert!(!std::fs::read_to_string(&included)
        .unwrap()
        .contains("Host second"));
    // Known hosts: real key data, batch removal and stale-row protection.
    std::fs::write(
        root.join("known_hosts"),
        format!(
            "first {}\nsecond {}\n",
            public.as_str().unwrap(),
            public.as_str().unwrap()
        ),
    )
    .unwrap();
    let rows = invoke("list_known_hosts", json!({}));
    invoke("delete_known_hosts", json!({"entries":[rows[0]]}));
    assert!(call("delete_known_hosts", json!({"entries":[rows[1]]})).is_err());
    assert_eq!(
        invoke("list_known_hosts", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // chmod operates on temporary fixtures and never expands stricter access.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join("team/work"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        std::fs::set_permissions(keypath, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(!invoke("audit_permissions", json!({}))
            .as_array()
            .unwrap()
            .is_empty());
        invoke("fix_permissions", json!({}));
        assert_eq!(
            std::fs::metadata(keypath).unwrap().permissions().mode() & 0o777,
            0o400
        );
        assert_eq!(
            std::fs::metadata(root.join("team/work"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(invoke("audit_permissions", json!({})), json!([]));
    }
    invoke(
        "save_app_config",
        json!({"config":{"preferred_terminal":null,"theme":"system"}}),
    );
    assert_eq!(invoke("get_app_config", json!({}))["theme"], "system");
    invoke("launch_ssh_connection", json!({"host_alias":"root-host"}));
    let captured =
        std::fs::read_to_string(std::env::var_os("SSH_GUI_CAPTURE_TERMINAL").unwrap()).unwrap();
    assert!(captured.contains("-F") && captured.contains(root.to_str().unwrap()));
    invoke(
        "copy_key_to_server",
        json!({"key_path":keypath,"host_alias":"root-host"}),
    );
    let captured =
        std::fs::read_to_string(std::env::var_os("SSH_GUI_CAPTURE_TERMINAL").unwrap()).unwrap();
    assert!(captured.contains("ssh-copy-id -f -F"));
    if std::env::var_os("SSH_GUI_LOCAL_NETWORK").is_some() {
        local_network_acceptance(root, keypath, public.as_str().unwrap(), &invoke);
    }
    invoke("add_key_to_agent", json!({"key_path":keypath}));
    assert!(call(
        "delete_ssh_key",
        json!({"key_path":keypath,"expected_fingerprint":"stale"})
    )
    .is_err());
    invoke(
        "delete_ssh_key",
        json!({"key_path":keypath,"expected_fingerprint":key["fingerprint"]}),
    );
    assert!(!Path::new(keypath).exists() && !Path::new(&format!("{keypath}.pub")).exists());
    assert_eq!(invoke("list_agent_keys", json!({})), json!([]));
    assert!(call(
        "delete_ssh_key",
        json!({"key_path":"/etc/passwd","expected_fingerprint":""})
    )
    .is_err());
    println!("LOCAL ACCEPTANCE PASSED: real IPC, keys, encrypted OpenSSH interoperability, agent, nested keys, Include edits, stale writes, recovery, known hosts, permissions, preferences and launcher commands");
}

fn local_network_acceptance(
    root: &Path,
    keypath: &str,
    public: &str,
    invoke: &impl Fn(&str, Value) -> Value,
) {
    let keygen = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(root.join("fixture-host"))
        .output()
        .unwrap();
    assert!(keygen.status.success());
    std::fs::write(root.join("accepted"), format!("{public}\n")).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let username = String::from_utf8(Command::new("id").arg("-un").output().unwrap().stdout)
        .unwrap()
        .trim()
        .to_string();
    let config = format!("ListenAddress 127.0.0.1\nPort {port}\nHostKey {}/fixture-host\nPidFile {}/sshd.pid\nAuthorizedKeysFile {}/accepted\nStrictModes no\nUsePAM no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\nAllowUsers {username}\nLogLevel ERROR\n", root.display(),root.display(),root.display());
    std::fs::write(root.join("sshd_config"), config).unwrap();
    let log = std::fs::File::create(root.join("sshd.log")).unwrap();
    let mut server = Process(
        Command::new("/usr/sbin/sshd")
            .args(["-D", "-e", "-f"])
            .arg(root.join("sshd_config"))
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "sshd exited: {}",
            std::fs::read_to_string(root.join("sshd.log")).unwrap()
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "sshd did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let host_public = std::fs::read_to_string(root.join("fixture-host.pub")).unwrap();
    let parts: Vec<_> = host_public.split_whitespace().collect();
    let hostname = format!("[127.0.0.1]:{port}");
    assert_eq!(
        invoke(
            "verify_known_host",
            json!({"hostname":hostname,"key_type":parts[0],"stored_key_data":parts[1]})
        ),
        true
    );
    assert_eq!(
        invoke(
            "verify_known_host",
            json!({"hostname":hostname,"key_type":parts[0],"stored_key_data":"different"})
        ),
        false
    );
    std::fs::write(
        root.join("fixture-known-hosts"),
        format!("{hostname} {host_public}"),
    )
    .unwrap();
    let client = root.join("fixture-client-config");
    std::fs::write(&client, format!("Host local-acceptance\n  HostName 127.0.0.1\n  Port {port}\n  User {username}\n  IdentityFile {keypath}\n  IdentitiesOnly yes\n  UserKnownHostsFile {}/fixture-known-hosts\n  StrictHostKeyChecking yes\n  BatchMode yes\n", root.display())).unwrap();
    let result = Command::new("ssh")
        .arg("-F")
        .arg(&client)
        .args(["local-acceptance", "printf", "ssh-gui-local-ok"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stderr),
        std::fs::read_to_string(root.join("sshd.log")).unwrap()
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout), "ssh-gui-local-ok");
    let install = invoke(
        "generate_ssh_key",
        json!({"params":{"algorithm":"Ed25519","filename":"install-fixture","comment":"install fixture","passphrase":null}}),
    );
    let previous_config = std::fs::read(root.join("config")).unwrap();
    let previous_known = std::fs::read(root.join("known_hosts")).unwrap();
    std::fs::write(
        root.join("config"),
        std::fs::read_to_string(&client)
            .unwrap()
            .replace(&format!("Port {port}"), "Port 9"),
    )
    .unwrap();
    std::fs::copy(root.join("fixture-known-hosts"), root.join("known_hosts")).unwrap();
    let success = invoke(
        "diagnose_ssh",
        json!({"alias":"local-acceptance","network":true,"port":port}),
    );
    assert_eq!(success["authenticated"], true, "{}", success);
    let wrong = std::fs::read_to_string(&client)
        .unwrap()
        .replace(keypath, install["private_path"].as_str().unwrap());
    std::fs::write(root.join("config"), wrong).unwrap();
    let rejected = invoke(
        "diagnose_ssh",
        json!({"alias":"local-acceptance","network":true}),
    );
    assert_eq!(rejected["authenticated"], false, "{}", rejected);
    assert!(rejected["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["title"] == "Authentication rejected"));
    std::fs::write(root.join("config"), &previous_config).unwrap();
    std::fs::write(root.join("known_hosts"), &previous_known).unwrap();
    // ssh-copy-id supports an explicit remote destination, so it writes only inside this fixture.
    let transfer = Command::new("ssh-copy-id")
        .arg("-f")
        .arg("-F")
        .arg(&client)
        .arg("-i")
        .arg(install["public_path"].as_str().unwrap())
        .arg("-t")
        .arg(root.join("accepted"))
        .arg("local-acceptance")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        transfer.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&transfer.stdout),
        String::from_utf8_lossy(&transfer.stderr)
    );
    let installed_auth = Command::new("ssh")
        .args([
            "-F",
            "/dev/null",
            "-i",
            install["private_path"].as_str().unwrap(),
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "BatchMode=yes",
            "-o",
        ])
        .arg(format!(
            "UserKnownHostsFile={}/fixture-known-hosts",
            root.display()
        ))
        .args(["-p", &port.to_string()])
        .arg(format!("{username}@127.0.0.1"))
        .args(["printf", "installed-key-ok"])
        .output()
        .unwrap();
    assert!(
        installed_auth.status.success(),
        "{}",
        String::from_utf8_lossy(&installed_auth.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&installed_auth.stdout),
        "installed-key-ok"
    );
    println!("LOOPBACK ACCEPTANCE PASSED: local SSH authentication, ssh-copy-id installation, new-key authentication and real host-key match/mismatch");
}
