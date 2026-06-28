//
//  ChatTimelineView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct ChatTimelineView: View {
    let title: String
    let subtitle: String
    let entries: [ChatEntry]

    var body: some View {
        VStack(spacing: 0) {
            ChatTimelineHeader(title: title, subtitle: subtitle)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    ForEach(entries) { entry in
                        ChatMessageView(entry: entry)
                    }
                }
                .padding(.horizontal, 22)
                .padding(.vertical, 18)
                .frame(maxWidth: 840)
                .frame(maxWidth: .infinity)
            }
            .background(ChatTimelineBackground())
        }
    }
}

private struct ChatTimelineHeader: View {
    let title: String
    let subtitle: String

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .center, spacing: 16) {
                ChatTimelineTitleBlock(title: title, subtitle: subtitle)

                Spacer()

                TimelineWideControls()
            }

            HStack(alignment: .center, spacing: 16) {
                ChatTimelineTitleBlock(title: title, subtitle: subtitle)

                Spacer()

                TimelineAllControlsMenu()
            }
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 14)
        .background(.regularMaterial)
        .overlay(alignment: .bottom) {
            Divider().opacity(0.6)
        }
    }
}

private struct ChatTimelineTitleBlock: View {
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title)
                .font(.system(size: 18, weight: .semibold, design: .rounded))
                .foregroundStyle(.primary)
                .lineLimit(1)

            Text(subtitle)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .layoutPriority(1)
    }
}

private struct TimelineWideControls: View {
    var body: some View {
        HStack(spacing: 6) {
            TimelineIconButton(title: "History", systemImage: "clock.arrow.circlepath")
            TimelineIconButton(title: "Compact", systemImage: "arrow.down.right.and.arrow.up.left")
            TimelineIconButton(title: "Processes", systemImage: "terminal")

            TimelineOperationsMenu()
        }
        .controlSize(.regular)
    }
}

private struct TimelineOperationsMenu: View {
    var body: some View {
        Menu {
            Button {
            } label: {
                Label("Role Admin", systemImage: "person.crop.rectangle")
            }

            Button {
            } label: {
                Label("Workflow Memory", systemImage: "list.bullet.rectangle")
            }

            Button {
            } label: {
                Label("Command Registry", systemImage: "command")
            }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .menuStyle(.button)
        .buttonStyle(.borderless)
        .help("More runtime operations")
    }
}

private struct TimelineAllControlsMenu: View {
    var body: some View {
        Menu {
            Button {
            } label: {
                Label("History", systemImage: "clock.arrow.circlepath")
            }

            Button {
            } label: {
                Label("Compact", systemImage: "arrow.down.right.and.arrow.up.left")
            }

            Button {
            } label: {
                Label("Processes", systemImage: "terminal")
            }

            Divider()

            Button {
            } label: {
                Label("Role Admin", systemImage: "person.crop.rectangle")
            }

            Button {
            } label: {
                Label("Workflow Memory", systemImage: "list.bullet.rectangle")
            }

            Button {
            } label: {
                Label("Command Registry", systemImage: "command")
            }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .menuStyle(.button)
        .buttonStyle(.borderless)
        .help("Runtime operations")
    }
}

private struct TimelineIconButton: View {
    let title: String
    let systemImage: String

    var body: some View {
        Button {
        } label: {
            Label(title, systemImage: systemImage)
                .labelStyle(.iconOnly)
        }
        .buttonStyle(.borderless)
        .help(title)
    }
}

private struct ChatTimelineBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.030, green: 0.040, blue: 0.056)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.030),
                    Color.clear,
                    Color.black.opacity(0.10)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let entries = [
        ChatEntry(
            id: "user-message",
            author: "You",
            detail: nil,
            time: "10:12",
            body: "Clean up the release notes and attach proof before handing this back.",
            output: nil,
            kind: .message,
            alignment: .trailing
        ),
        ChatEntry(
            id: "assistant-message",
            author: "Runtime allow",
            detail: nil,
            time: "10:12",
            body: "I’ll review the current notes, make the smallest safe edit, then capture evidence for Requirements review.",
            output: nil,
            kind: .message,
            alignment: .leading
        ),
        ChatEntry(
            id: "tool-result",
            author: "Code run",
            detail: "completed",
            time: "10:13",
            body: "Validated the edited files and collected the relevant output.",
            output: "2 files changed\nrequirements evidence ready",
            kind: .toolResult,
            alignment: .leading
        ),
        ChatEntry(
            id: "approval",
            author: "Approval needed",
            detail: "owner action",
            time: "10:16",
            body: "This action needs approval before the runtime can continue.",
            output: nil,
            kind: .approval,
            alignment: .leading
        )
    ]

    ChatTimelineView(
        title: "Release notes cleanup",
        subtitle: "Runtime allow · Idle",
        entries: entries
    )
        .frame(width: 820, height: 760)
        .preferredColorScheme(.dark)
}
