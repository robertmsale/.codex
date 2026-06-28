//
//  SessionListView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct SessionListView: View {
    let groups: [SessionGroup]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Sessions")
                        .font(.system(size: 17, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Text("Select a runtime session")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button {
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .help("New session")
            }
            .padding(.horizontal, 14)
            .padding(.top, 14)

            List {
                ForEach(groups) { group in
                    Section {
                        ForEach(group.sessions) { session in
                            SessionRowView(session: session)
                                .listRowInsets(EdgeInsets(top: 2, leading: 10, bottom: 2, trailing: 10))
                                .listRowSeparator(.hidden)
                                .listRowBackground(Color.clear)
                        }
                    } header: {
                        Text(group.title)
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(.secondary)
                            .textCase(.uppercase)
                            .tracking(1.1)
                            .padding(.top, 10)
                            .padding(.leading, 5)
                    }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
        }
        .background(SessionListBackground())
    }
}

private struct SessionRowView: View {
    let session: SessionItem

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            StatusMark(tone: session.tone)
                .padding(.top, 5)

            VStack(alignment: .leading, spacing: 5) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(session.title)
                        .font(.system(size: 14, weight: session.selected ? .semibold : .medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)

                    Spacer(minLength: 8)

                    Text(session.state)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(session.tone.foreground)
                        .lineLimit(1)
                }

                Text(session.summary)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            if session.selected {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(Color.primary.opacity(0.075))
            }
        }
        .overlay {
            if session.selected {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(Color.primary.opacity(0.12), lineWidth: 1)
            }
        }
        .contentShape(Rectangle())
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            Button(role: .destructive) {
            } label: {
                Label("Archive", systemImage: "trash")
            }

            Button {
            } label: {
                Label("Fork", systemImage: "arrow.triangle.branch")
            }
            .tint(.blue)

            Button {
            } label: {
                Label("Settings", systemImage: "gearshape")
            }
            .tint(.gray)
        }
        .contextMenu {
            Button {
            } label: {
                Label("Settings", systemImage: "gearshape")
            }

            Button {
            } label: {
                Label("Fork", systemImage: "arrow.triangle.branch")
            }

            Divider()

            Button(role: .destructive) {
            } label: {
                Label("Archive", systemImage: "trash")
            }
        }
    }
}

private struct StatusMark: View {
    let tone: SessionTone

    var body: some View {
        Circle()
            .fill(tone.foreground)
            .frame(width: 7, height: 7)
            .shadow(color: tone.foreground.opacity(0.35), radius: 5, x: 0, y: 0)
    }
}

private struct SessionListBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.045, green: 0.060, blue: 0.078)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.055),
                    Color.white.opacity(0.018),
                    Color.black.opacity(0.10)
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
    }
}

struct SessionGroup: Identifiable {
    let id: String
    let title: String
    let sessions: [SessionItem]
}

struct SessionItem: Identifiable {
    let id: String
    let title: String
    let summary: String
    let state: String
    let tone: SessionTone
    let selected: Bool
}

enum SessionTone {
    case running
    case attention
    case idle
    case failed

    var foreground: Color {
        switch self {
        case .running:
            return Color(red: 0.47, green: 0.86, blue: 0.62)
        case .attention:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .idle:
            return Color(red: 0.62, green: 0.70, blue: 0.80)
        case .failed:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }
}

#Preview(traits: .landscapeRight) {
    let groups = [
        SessionGroup(
            id: "needs-attention",
            title: "Needs attention",
            sessions: [
                SessionItem(
                    id: "approval",
                    title: "Release notes cleanup",
                    summary: "Approval required before changing files outside the workspace.",
                    state: "Approval",
                    tone: .attention,
                    selected: true
                ),
                SessionItem(
                    id: "command-review",
                    title: "CLI install check",
                    summary: "A requested command needs review before it can run.",
                    state: "Review",
                    tone: .attention,
                    selected: false
                )
            ]
        ),
        SessionGroup(
            id: "active",
            title: "Active",
            sessions: [
                SessionItem(
                    id: "starter-kit",
                    title: "Starter kit evidence",
                    summary: "Generating preview evidence and checking Requirements claims.",
                    state: "Running",
                    tone: .running,
                    selected: false
                )
            ]
        ),
        SessionGroup(
            id: "idle",
            title: "Idle",
            sessions: [
                SessionItem(
                    id: "role-admin",
                    title: "Role admin pass",
                    summary: "Ready for the next instruction.",
                    state: "Idle",
                    tone: .idle,
                    selected: false
                ),
                SessionItem(
                    id: "runtime-docs",
                    title: "Runtime docs sweep",
                    summary: "No action needed.",
                    state: "Idle",
                    tone: .idle,
                    selected: false
                )
            ]
        )
    ]

    SessionListView(groups: groups)
        .frame(width: 320, height: 720)
        .preferredColorScheme(.dark)
}
