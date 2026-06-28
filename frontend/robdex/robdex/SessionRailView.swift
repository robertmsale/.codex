//
//  SessionRailView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct SessionRailView: View {
    let title: String
    let status: String
    let statusColor: Color
    let projects: [ProjectItem]
    let sessionGroups: [SessionGroup]

    var body: some View {
        GeometryReader { proxy in
            if proxy.size.width < 520 {
                CompactSessionRailView(title: title, status: status, statusColor: statusColor, projects: projects, sessionGroups: sessionGroups)
            } else {
                WideSessionRailView(title: title, status: status, statusColor: statusColor, projects: projects, sessionGroups: sessionGroups)
            }
        }
        .background(RailBackground())
    }
}

private struct WideSessionRailView: View {
    let title: String
    let status: String
    let statusColor: Color
    let projects: [ProjectItem]
    let sessionGroups: [SessionGroup]

    var body: some View {
        VStack(spacing: 0) {
            RailHeader(title: title, status: status, statusColor: statusColor)

            ProjectListView(projects: projects)
                .frame(height: 270)

            Divider()
                .opacity(0.65)

            SessionListView(groups: sessionGroups)
        }
    }
}

private struct CompactSessionRailView: View {
    let title: String
    let status: String
    let statusColor: Color
    let projects: [ProjectItem]
    let sessionGroups: [SessionGroup]

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                RailHeader(title: title, status: status, statusColor: statusColor)
                    .padding(.bottom, 2)

                ProjectListView(
                    projects: projects,
                    navigatesToSessions: true,
                    sessionGroups: sessionGroups
                )
            }
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

private struct RailHeader: View {
    let title: String
    let status: String
    let statusColor: Color

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text(status)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(statusColor)
            }

            Spacer()

            Button {
            } label: {
                Image(systemName: "plus.message")
            }
            .buttonStyle(.borderless)
            .help("New session")
        }
        .padding(.horizontal, 14)
        .padding(.top, 14)
        .padding(.bottom, 12)
    }
}

private struct RailBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.045, green: 0.060, blue: 0.078)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.055),
                    Color.white.opacity(0.016),
                    Color.black.opacity(0.12)
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
        .ignoresSafeArea()
    }
}

#Preview(traits: .landscapeLeft) {
    let projects = [
        ProjectItem(
            id: "runtime",
            title: "Runtime",
            summary: "Default Agent Runtime scope",
            selected: true
        ),
        ProjectItem(
            id: "starter-kit",
            title: "Starter Kit",
            summary: "App scaffold and evidence work",
            selected: false
        ),
        ProjectItem(
            id: "codex-home",
            title: "Codex Home",
            summary: "Local orchestration workspace",
            selected: false
        ),
        ProjectItem(
            id: "unassigned",
            title: "Unassigned",
            summary: "Sessions without a project",
            selected: false
        )
    ]

    let sessionGroups = [
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
                )
            ]
        )
    ]

    SessionRailView(
        title: "Agent Runtime",
        status: "Connected",
        statusColor: Color(red: 0.47, green: 0.86, blue: 0.62),
        projects: projects,
        sessionGroups: sessionGroups
    )
        .frame(width: 360, height: 760)
        .preferredColorScheme(.dark)
}
