import SwiftUI
import MacoraCore

@MainActor struct CommandPalette: View {
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var selected = 0
    @FocusState private var focused: Bool
    let perform: (WorkspaceCommand) -> Void
    private var commands: [WorkspaceCommand] { WorkspaceCatalog.filtered(query) }
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField("Where would you like to go?", text: $query).font(.title3).textFieldStyle(.plain).focused($focused)
                    .onSubmit { choose() }
                    .onKeyPress(.downArrow) { selected = min(selected + 1, max(0, commands.count - 1)); return .handled }
                    .onKeyPress(.upArrow) { selected = max(0, selected - 1); return .handled }
                Text("⌘K").font(.caption.monospaced()).foregroundStyle(.secondary)
            }.padding(22)
            Divider()
            ScrollViewReader { reader in
                ScrollView {
                    LazyVStack(spacing: 4) {
                        ForEach(Array(commands.enumerated()), id: \.element.id) { index, command in
                            Button { dismiss(); perform(command) } label: {
                                HStack(spacing: 14) {
                                    Image(systemName: command.symbol).foregroundStyle(MacoraDesign.blue).frame(width: 24)
                                    VStack(alignment: .leading, spacing: 4) { Text(command.title).font(.body.weight(.medium)); Text(command.detail).font(.caption).foregroundStyle(.secondary) }
                                    Spacer(); if index == selected { Image(systemName: "return").foregroundStyle(.secondary) }
                                }.padding(14).frame(maxWidth: .infinity, alignment: .leading).background(index == selected ? MacoraDesign.blue.opacity(0.10) : .clear, in: RoundedRectangle(cornerRadius: 12))
                            }.buttonStyle(.plain).id(index)
                        }
                        if commands.isEmpty { ContentUnavailableView.search(text: query) }
                    }.padding(12)
                }.onChange(of: selected) { _, index in reader.scrollTo(index) }
            }
            Divider()
            HStack { Text("↑ ↓ navigate"); Text("↵ open"); Spacer(); Button("Close") { dismiss() }.keyboardShortcut(.cancelAction) }.font(.caption).foregroundStyle(.secondary).padding(14)
        }.frame(width: 650, height: 510).task { focused = true }.onChange(of: query) { _, _ in selected = 0 }
    }
    private func choose() { guard commands.indices.contains(selected) else { return }; let command = commands[selected]; dismiss(); perform(command) }
}
