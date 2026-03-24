#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::todo)]
#![deny(clippy::unreachable)]
#![deny(clippy::allow_attributes_without_reason)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest};
use ashpd::desktop::global_shortcuts::{
  BindShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::{CreateSessionOptions, Session};
use gtk::glib;
use gtk::prelude::*;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

struct SoundBinding {
  id: String,
  data: Vec<u8>,
}

struct AppState {
  bindings: Vec<SoundBinding>,
  audio_sink: MixerDeviceSink,
  #[allow(dead_code, reason = "need to keep it somewhere")]
  audio_player: Player,
  global_shortcuts: Option<GlobalShortcuts>,
  session: Option<Session<GlobalShortcuts>>,
}

impl AppState {
  fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let audio_sink = DeviceSinkBuilder::open_default_sink()?;
    let audio_player = Player::connect_new(&audio_sink.mixer());
    Ok(Self {
      bindings: Vec::new(),
      audio_sink,
      audio_player,
      global_shortcuts: None,
      session: None,
    })
  }

  fn play_sound(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let cursor = std::io::Cursor::new(data.to_vec());
    let source = Decoder::new(cursor)?;
    self.audio_sink.mixer().add(source);
    Ok(())
  }
}

fn main() -> glib::ExitCode {
  let app = gtk::Application::builder()
    .application_id("com.github.haras-unicorn.fahhh")
    .build();

  app.connect_activate(build_ui);

  app.run()
}

fn build_ui(app: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .application(app)
    .title("Fahhh - Soundboard")
    .default_width(400)
    .default_height(300)
    .resizable(true)
    .build();

  window.connect_close_request(move |win| {
    let Some(app) = win.application() else {
      return glib::Propagation::Proceed;
    };

    app.quit();
    return glib::Propagation::Stop;
  });

  let vbox = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(12)
    .margin_bottom(12)
    .margin_start(12)
    .margin_end(12)
    .build();

  let title = gtk::Label::builder()
    .label("Soundboard")
    .css_classes(vec!["title-1".to_string()])
    .build();

  let scrolled = gtk::ScrolledWindow::builder()
    .hexpand(true)
    .vexpand(true)
    .build();

  let list_box = gtk::ListBox::builder()
    .css_classes(vec!["boxed-list".to_string()])
    .build();
  scrolled.set_child(Some(&list_box));

  let add_button = gtk::Button::builder()
    .label("Add Sound")
    .css_classes(vec!["suggested-action".to_string()])
    .halign(gtk::Align::Center)
    .build();

  vbox.append(&title);
  vbox.append(&scrolled);
  vbox.append(&add_button);

  window.set_child(Some(&vbox));

  let state = Rc::new(RefCell::new(AppState::new().unwrap_or_else(|e| {
    eprintln!("Failed to initialize audio: {}", e);
    std::process::exit(1);
  })));

  let state_clone = state.clone();
  glib::spawn_future_local(async move {
    if let Err(e) = request_permissions(state_clone.clone()).await {
      eprintln!("Failed to get permissions: {}", e);
    }
  });

  let state_clone = state.clone();
  let list_box_clone = list_box.clone();
  add_button.connect_clicked(move |btn| {
    eprintln!(
      "Button clicked! Sensitive: {}, Has focus: {}",
      btn.is_sensitive(),
      btn.has_focus()
    );
    let state = state_clone.clone();
    let list_box = list_box_clone.clone();
    glib::spawn_future_local(async move {
      eprintln!("Async button clicked!");
      if let Err(e) = add_sound_dialog(state, list_box).await {
        eprintln!("Failed to add sound: {}", e);
      }
      eprintln!("File chosen");
    });
  });

  window.present();
}

async fn request_permissions(
  state: Rc<RefCell<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
  let shortcuts = GlobalShortcuts::new().await?;
  let session = shortcuts
    .create_session(CreateSessionOptions::default())
    .await?;

  let proxy = shortcuts.clone();
  {
    let mut state = state.borrow_mut();
    state.global_shortcuts = Some(shortcuts);
    state.session = Some(session);
  }

  glib::spawn_future_local(async move {
    loop {
      match proxy.receive_all_signals().await {
        Ok(activated) => {
          if let Ok(state) = state.try_borrow()
            && let Some(shortcut_id) = activated.name()
          {
            for binding in &state.bindings {
              if binding.id == shortcut_id.to_string() {
                let _ = state.play_sound(&binding.data);
                break;
              }
            }
          }
        }
        Err(e) => {
          eprintln!("Error receiving shortcut activation: {}", e);
          break;
        }
      }
    }
  });

  Ok(())
}

async fn add_sound_dialog(
  state: Rc<RefCell<AppState>>,
  list_box: gtk::ListBox,
) -> Result<(), Box<dyn std::error::Error>> {
  eprintln!("Adding sound dialog!");

  let connection = ashpd::zbus::Connection::session().await?;
  eprintln!("Connection to DBus established!");

  let reply = connection
    .call_method(
      Some("org.freedesktop.portal.Desktop"),
      "/org/freedesktop/portal/desktop",
      Some("org.freedesktop.DBus.Peer"),
      "Ping",
      &(),
    )
    .await?;
  eprintln!("DBus ping: {:?}", reply);

  let files = OpenFileRequest::default()
    .multiple(false)
    .modal(true)
    .title("Select Sound File")
    .filters(vec![FileFilter::new("Audio Files").mimetype("audio/*")])
    .send()
    .await?;

  let files = files.response()?;
  eprintln!("Selected files: {files:?}");

  if let Some(path) = files
    .uris()
    .first()
    .and_then(|uri| uri.as_str().split_once(':'))
    .map(|(_, path)| Path::new(path))
  {
    let data = std::fs::read(&path)?;
    let file_name = path
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("Unknown")
      .to_string();

    let shortcut_id = format!("sound_{}", state.borrow().bindings.len());

    let binding = SoundBinding {
      id: shortcut_id.clone(),
      data,
    };

    state.borrow_mut().bindings.push(binding);

    {
      let state = state.borrow();
      if let Some(ref proxy) = state.global_shortcuts
        && let Some(ref session) = state.session
      {
        let _ = proxy
          .bind_shortcuts(
            &session,
            &[NewShortcut::new(shortcut_id.clone(), file_name.clone())],
            None,
            BindShortcutsOptions::default(),
          )
          .await;
      }
    }

    add_sound_row(&list_box, file_name, shortcut_id);
  }

  Ok(())
}

fn add_sound_row(list_box: &gtk::ListBox, name: String, shortcut_id: String) {
  let row = gtk::ListBoxRow::builder()
    .activatable(false)
    .selectable(false)
    .build();

  let hbox = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(12)
    .margin_top(6)
    .margin_bottom(6)
    .margin_start(6)
    .margin_end(6)
    .build();

  let name_label = gtk::Label::builder()
    .label(&name)
    .hexpand(true)
    .halign(gtk::Align::Start)
    .build();

  let shortcut_label = gtk::Label::builder()
    .label(&format!("ID: {}", shortcut_id))
    .halign(gtk::Align::End)
    .css_classes(vec!["dim-label".to_string()])
    .build();

  hbox.append(&name_label);
  hbox.append(&shortcut_label);
  row.set_child(Some(&hbox));

  list_box.append(&row);
}
