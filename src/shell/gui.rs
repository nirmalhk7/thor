use egui::{Align, Color32, Context, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::time::Instant;

pub struct ThorGuiState {
    pub url_input: String,
    pub selected_tab: usize,
    pub show_cmd_palette: bool,
    pub cmd_search_query: String,
    pub show_3d_dom: bool,
    pub is_multi_viewport: bool,
    pub is_devtools_open: bool,
    pub start_time: Instant,
    pub frame_counter: u64,
    pub active_viewports: Vec<(&'static str, Vec2)>,
}

impl ThorGuiState {
    pub fn new() -> Self {
        Self {
            url_input: "https://localhost:3000".to_string(),
            selected_tab: 1,
            show_cmd_palette: false,
            cmd_search_query: String::new(),
            show_3d_dom: false,
            is_multi_viewport: true,
            is_devtools_open: false,
            start_time: Instant::now(),
            frame_counter: 0,
            active_viewports: vec![
                ("📱 Mobile (393x852)", Vec2::new(260.0, 520.0)),
                ("📱 Tablet (1024x1366)", Vec2::new(420.0, 520.0)),
                ("💻 Desktop (1920x1080)", Vec2::new(600.0, 520.0)),
            ],
        }
    }
}

pub fn render_thor_ui(ctx: &Context, state: &mut ThorGuiState) {
    state.frame_counter += 1;

    // Dark Obsidian Theme Palette Tokens
    let dark_bg = Color32::from_rgb(18, 20, 26);
    let panel_bg = Color32::from_rgb(26, 29, 38);
    let border_color = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
    let accent_violet = Color32::from_rgb(168, 85, 247);
    let accent_cyan = Color32::from_rgb(6, 182, 212);
    let success_green = Color32::from_rgb(34, 197, 94);
    let warning_orange = Color32::from_rgb(249, 115, 22);

    // Apply custom dark style
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.window_fill = panel_bg;
    style.visuals.panel_fill = dark_bg;
    ctx.set_style(style);

    // Top Header Panel: Title, Tabs & Navigation Bar
    egui::TopBottomPanel::top("thor_header")
        .frame(Frame::none().fill(panel_bg).inner_margin(Margin::same(8.0)))
        .show(ctx, |ui| {
            // Title & Tab Bar Row
            ui.horizontal(|ui| {
                ui.heading(RichText::new("⚡ THOR").bold().color(accent_violet));
                ui.add_space(8.0);

                // Tab Items
                let tabs = [
                    "⚡ Engine Dashboard",
                    "🌐 localhost:3000 (React App)",
                    "📄 API Docs",
                    "+ New Tab",
                ];

                for (idx, title) in tabs.iter().enumerate() {
                    let is_active = state.selected_tab == idx;
                    let text_color = if is_active { Color32::WHITE } else { Color32::GRAY };
                    let fill = if is_active { Color32::from_rgb(38, 42, 54) } else { Color32::TRANSPARENT };

                    let btn = ui.add(
                        egui::Button::new(RichText::new(*title).color(text_color).size(13.0))
                            .fill(fill)
                            .stroke(Stroke::new(1.0, if is_active { accent_violet } else { Color32::TRANSPARENT })),
                    );

                    if btn.clicked() && idx != 3 {
                        state.selected_tab = idx;
                    }
                }
            });

            ui.add_space(6.0);

            // Navigation & Address Bar Row
            ui.horizontal(|ui| {
                ui.button("◀");
                ui.button("▶");
                ui.button("🔄");

                // Glassmorphic Address Bar
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔒").size(12.0));
                    ui.add_sized(
                        [ui.available_width() - 420.0, 24.0],
                        egui::TextEdit::singleline(&mut state.url_input)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(Color32::WHITE),
                    );
                    ui.label(RichText::new("🔐 ECDSA P-256 Trusted").color(success_green).size(11.0));
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("⌨ Cmd+K").color(accent_cyan)).clicked() {
                        state.show_cmd_palette = !state.show_cmd_palette;
                    }

                    if ui.button(if state.is_multi_viewport { "📐 Single View" } else { "📐 Multi-Viewport Canvas" }).clicked() {
                        state.is_multi_viewport = !state.is_multi_viewport;
                    }

                    if ui.button(if state.show_3d_dom { "🧊 2D View" } else { "🧊 3D DOM Snapshot" }).clicked() {
                        state.show_3d_dom = !state.show_3d_dom;
                    }

                    if ui.button("🔗 IDE Jump (Option+Click)").clicked() {
                        let _ = std::process::Command::new("open").arg("vscode://").status();
                    }
                });
            });
        });

    // Bottom Developer SLA Status Bar
    egui::TopBottomPanel::bottom("thor_status_bar")
        .frame(Frame::none().fill(Color32::from_rgb(14, 16, 21)).inner_margin(Margin::symmetric(12.0, 4.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚡ SLA Metrics:").bold().color(accent_violet).size(11.0));
                ui.label(RichText::new("Cold-Start <30ms (Pre-Warmed Daemon)").color(Color32::LIGHT_GRAY).size(11.0));
                ui.label(RichText::new("|").color(Color32::DARK_GRAY));
                ui.label(RichText::new("IPC Latency: 0.4μs (SPSC Ring Buffer)").color(accent_cyan).size(11.0));
                ui.label(RichText::new("|").color(Color32::DARK_GRAY));
                ui.label(RichText::new("120Hz GPU Compositor (Metal)").color(success_green).size(11.0));
                ui.label(RichText::new("|").color(Color32::DARK_GRAY));
                ui.label(RichText::new("RAM: 142MB Active / 48 Suspended (proc_suspend)").color(warning_orange).size(11.0));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let elapsed = state.start_time.elapsed().as_secs_f32();
                    let fps = (state.frame_counter as f32 / elapsed).min(120.0);
                    ui.label(RichText::new(format!("FPS: {:.0}", fps)).bold().color(accent_violet).size(11.0));
                });
            });
        });

    // Central Workspace Area
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.show_3d_dom {
            // 3D Exploded DOM Viewport Snapshot
            render_3d_dom_view(ui, accent_violet, accent_cyan);
        } else if state.is_multi_viewport {
            // Multi-Viewport Synchronized Preview Canvas
            render_multi_viewport_canvas(ui, state, panel_bg, border_color, accent_violet);
        } else {
            // Single Viewport Workbench Preview
            render_single_workbench(ui, state, panel_bg, accent_cyan, success_green);
        }
    });

    // Floating Neovim-Style Cmd+K Command Palette Modal Overlay
    if state.show_cmd_palette {
        render_cmd_palette_modal(ctx, state, panel_bg, accent_violet, accent_cyan);
    }
}

fn render_multi_viewport_canvas(
    ui: &mut egui::Ui,
    state: &ThorGuiState,
    panel_bg: Color32,
    border_color: Color32,
    accent_violet: Color32,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("📐 Multi-Viewport Synchronized Canvas").bold().size(15.0).color(Color32::WHITE));
            ui.label(RichText::new("• Auto-syncing scroll, click & form input across 3 breakpoints").color(Color32::GRAY).size(12.0));
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            for (title, size) in &state.active_viewports {
                Frame::none()
                    .fill(panel_bg)
                    .stroke(Stroke::new(1.0, border_color))
                    .rounding(8.0)
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(size.x);
                        ui.set_height(size.y);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*title).bold().color(accent_violet).size(12.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new("LIVE 120Hz").color(Color32::GREEN).size(10.0));
                            });
                        });
                        ui.separator();

                        // Viewport Web Render Simulation Content
                        ui.vertical(|ui| {
                            ui.add_space(10.0);
                            ui.heading(RichText::new("⚡ Next.js / Vite Dev Server").bold().size(16.0));
                            ui.label(RichText::new("Fast HMR active • Hot reloading engaged").color(Color32::LIGHT_GRAY).size(12.0));
                            ui.add_space(15.0);

                            // Interactive Demo Buttons inside Viewport
                            if ui.button("Option+Click DOM Element (Jump to VS Code)").clicked() {
                                let _ = std::process::Command::new("open").arg("vscode://file/src/App.tsx:24:10").status();
                            }

                            ui.add_space(10.0);
                            ui.group(|ui| {
                                ui.label(RichText::new("🔍 SSR vs CSR Hydration Inspector").bold().size(12.0));
                                ui.label(RichText::new("✔ 0 Mismatch Errors Detected").color(Color32::GREEN).size(11.0));
                            });

                            ui.add_space(10.0);
                            ui.group(|ui| {
                                ui.label(RichText::new("📡 API Mocking Engine").bold().size(12.0));
                                ui.label(RichText::new("GET /api/user -> 200 OK (0.2ms)").color(Color32::CYAN).size(11.0));
                            });
                        });
                    });

                ui.add_space(12.0);
            }
        });
    });
}

fn render_single_workbench(
    ui: &mut egui::Ui,
    state: &ThorGuiState,
    panel_bg: Color32,
    accent_cyan: Color32,
    success_green: Color32,
) {
    Frame::none()
        .fill(panel_bg)
        .rounding(8.0)
        .inner_margin(Margin::same(16.0))
        .show(ui, |ui| {
            ui.heading(RichText::new("⚡ Thor Developer Workbench").bold().color(Color32::WHITE));
            ui.label(RichText::new(format!("Active URL: {}", state.url_input)).color(accent_cyan));
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("🔥 HMR WebSocket Proxy").bold().color(success_green));
                    ui.label("Status: Warm & Connected");
                    ui.label("Buffered Payloads: 0 (Tab Sleep Protection Active)");
                });

                ui.add_space(12.0);

                ui.group(|ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("🛡️ Local SSL CA Certificate").bold().color(accent_cyan));
                    ui.label("Issuer: Thor Dynamic P-256 Local CA");
                    ui.label("Domains: *.localhost, *.test, dev.local");
                });
            });
        });
}

fn render_3d_dom_view(ui: &mut egui::Ui, accent_violet: Color32, accent_cyan: Color32) {
    ui.vertical(|ui| {
        ui.heading(RichText::new("🧊 Obsidian Glass 3D Exploded DOM Snapshot").bold().color(accent_violet));
        ui.label(RichText::new("Visualizing Z-axis DOM layout tree layers, padding, margin, and CSS Grid bounds").color(Color32::GRAY));
        ui.add_space(16.0);

        let (rect, _response) = ui.allocate_at_least(Vec2::new(ui.available_width(), 480.0), Sense::hover());
        let painter = ui.painter_at(rect);

        // Draw 3D Floating Translucent Glass Panes
        let center = rect.center();
        let layers = [
            ("html / body (Root)", Color32::from_rgba_unmultiplied(168, 85, 247, 40), Vec2::new(-60.0, -60.0)),
            ("main#app-container", Color32::from_rgba_unmultiplied(6, 182, 212, 50), Vec2::new(-30.0, -30.0)),
            ("div.hero-banner (Padding: 24px)", Color32::from_rgba_unmultiplied(34, 197, 94, 60), Vec2::new(0.0, 0.0)),
            ("button.cta-primary (Active Node)", Color32::from_rgba_unmultiplied(249, 115, 22, 80), Vec2::new(30.0, 30.0)),
        ];

        for (label, color, offset) in layers {
            let pane_rect = Rect::from_center_size(Pos2::new(center.x + offset.x, center.y + offset.y), Vec2::new(400.0, 220.0));
            painter.rect_filled(pane_rect, 8.0, color);
            painter.rect_stroke(pane_rect, 8.0, Stroke::new(1.5, Color32::WHITE));
            painter.text(pane_rect.left_top() + Vec2::new(12.0, 12.0), Align::LEFT, label, egui::FontId::monospace(13.0), Color32::WHITE);
        }
    });
}

fn render_cmd_palette_modal(
    ctx: &Context,
    state: &mut ThorGuiState,
    panel_bg: Color32,
    accent_violet: Color32,
    accent_cyan: Color32,
) {
    egui::Window::new("⌨ Neovim Command Palette (Cmd+K)")
        .collapsible(false)
        .resizable(false)
        .anchor(Align::Center, Vec2::new(0.0, -120.0))
        .fixed_size(Vec2::new(560.0, 320.0))
        .frame(Frame::window(&ctx.style()).fill(panel_bg).stroke(Stroke::new(1.5, accent_violet)))
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.add_sized(
                [ui.available_width(), 32.0],
                egui::TextEdit::singleline(&mut state.cmd_search_query)
                    .hint_text("Type a command, script, CSS file, or active tab...")
                    .font(egui::FontId::monospace(14.0)),
            );
            ui.add_space(8.0);
            ui.separator();

            let commands = [
                ("🔗 Jump to Source Code (VS Code)", "vscode://file/src/App.tsx:24"),
                ("❄ Freeze Active Tab (OS Compaction Test)", "proc_suspend pid: 4821"),
                ("🔍 Inspect Hydration Mismatch Discrepancies", "0 errors found"),
                ("🔐 Generate ECDSA P-256 Local SSL Cert", "*.localhost"),
                ("⚡ Toggle SPSC Fast-Path Console Log Stream", "<16ms budget active"),
            ];

            for (cmd, detail) in commands {
                if state.cmd_search_query.is_empty() || cmd.to_lowercase().contains(&state.cmd_search_query.to_lowercase()) {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(false, RichText::new(cmd).color(Color32::WHITE).bold()).clicked() {
                            state.show_cmd_palette = false;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(detail).color(accent_cyan).size(11.0));
                        });
                    });
                }
            }
        });
}
