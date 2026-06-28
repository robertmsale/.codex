//
//  ProjectListView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct ProjectListView: View {
    let projects: [ProjectItem]
    let navigatesToSessions: Bool
    let sessionGroups: [SessionGroup]

    init(
        projects: [ProjectItem],
        navigatesToSessions: Bool = false,
        sessionGroups: [SessionGroup] = []
    ) {
        self.projects = projects
        self.navigatesToSessions = navigatesToSessions
        self.sessionGroups = sessionGroups
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Projects")
                        .font(.system(size: 13, weight: .semibold, design: .rounded))
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                        .tracking(1.2)

                    Text("Runtime scope")
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                }

                Spacer()

                Button {
                } label: {
                    Image(systemName: "folder.badge.plus")
                }
                .buttonStyle(.borderless)
                .help("New project")
            }
            .padding(.horizontal, 14)
            .padding(.top, 14)

            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(projects) { project in
                        if navigatesToSessions {
                            NavigationLink {
                                SessionListView(groups: sessionGroups)
                                    .navigationTitle(project.title)
                            } label: {
                                ProjectRowView(project: project)
                            }
                            .buttonStyle(.plain)
                        } else {
                            ProjectRowView(project: project)
                        }
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 14)
            }
        }
        .background(ProjectListBackground())
    }
}

private struct ProjectRowView: View {
    let project: ProjectItem

    var body: some View {
        HStack(alignment: .center, spacing: 11) {
            ProjectGlyph(title: project.title, selected: project.selected)

            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(project.title)
                        .font(.system(size: 14, weight: project.selected ? .semibold : .medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)

                    Spacer(minLength: 8)

                    if project.selected {
                        Text("Current")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(Color(red: 0.98, green: 0.67, blue: 0.25))
                    }
                }

                Text(project.summary)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
        .background {
            if project.selected {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(Color.primary.opacity(0.075))
            }
        }
        .overlay {
            if project.selected {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(Color.primary.opacity(0.12), lineWidth: 1)
            }
        }
        .contentShape(Rectangle())
    }
}

private struct ProjectGlyph: View {
    let title: String
    let selected: Bool

    var initials: String {
        let words = title.split(separator: " ")
        if words.count >= 2 {
            return String(words[0].prefix(1) + words[1].prefix(1)).uppercased()
        }
        return String(title.prefix(2)).uppercased()
    }

    var body: some View {
        Text(initials)
            .font(.system(size: 11, weight: .bold, design: .rounded))
            .foregroundStyle(selected ? Color.black.opacity(0.82) : Color.primary.opacity(0.78))
            .frame(width: 30, height: 30)
            .background {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(selected ? Color(red: 0.98, green: 0.67, blue: 0.25) : Color.primary.opacity(0.08))
            }
            .overlay {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(Color.primary.opacity(selected ? 0.0 : 0.10), lineWidth: 1)
            }
    }
}

private struct ProjectListBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.045, green: 0.060, blue: 0.078)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.050),
                    Color.white.opacity(0.014),
                    Color.black.opacity(0.08)
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
    }
}

struct ProjectItem: Identifiable {
    let id: String
    let title: String
    let summary: String
    let selected: Bool
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

    ProjectListView(projects: projects)
        .frame(width: 320, height: 280)
        .preferredColorScheme(.dark)
}
