//
//  RoleEditorView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct RoleEditorView: View {
    let roles: [RuntimeRole]
    let actions: [RoleActionAuthority]
    let validationMessages: [String]
    let runtimeError: RoleEditorRuntimeError?
    let onClose: () -> Void
    let onCreateRole: () -> Void
    let onValidate: (RoleEditorDraft) -> Void
    let onSaveVersion: (RoleEditorDraft) -> Void
    let onArchiveRole: (String) -> Void
    let onUnarchiveRole: (String) -> Void
    let onActivateVersion: (String) -> Void
    let onExportVersion: (String) -> Void

    @State private var selectedRoleId: String
    @State private var selectedVersionId: String
    @State private var label: String
    @State private var descriptionText: String
    @State private var instructionText: String
    @State private var authorityDecisions: [String: RolePolicyDecision]
    @State private var submitted = false

    private var selectedRole: RuntimeRole? {
        roles.first(where: { $0.id == selectedRoleId })
    }

    private var draft: RoleEditorDraft {
        RoleEditorDraft(
            roleId: selectedRoleId,
            label: label.trimmingCharacters(in: .whitespacesAndNewlines),
            descriptionText: descriptionText.trimmingCharacters(in: .whitespacesAndNewlines),
            instructionText: instructionText.trimmingCharacters(in: .whitespacesAndNewlines),
            authorityDecisions: authorityDecisions
        )
    }

    private var localValidationMessages: [String] {
        var messages: [String] = []
        if draft.roleId.isEmpty {
            messages.append("Select or create a role before saving.")
        }
        if draft.label.isEmpty {
            messages.append("Role label is required.")
        }
        if draft.instructionText.isEmpty {
            messages.append("Instruction text is required.")
        }
        return messages
    }

    private var visibleValidationMessages: [String] {
        localValidationMessages + validationMessages
    }

    private var canSave: Bool {
        localValidationMessages.isEmpty && selectedRole != nil
    }

    init(
        roles: [RuntimeRole],
        actions: [RoleActionAuthority],
        selectedRoleId: String? = nil,
        validationMessages: [String] = [],
        runtimeError: RoleEditorRuntimeError? = nil,
        onClose: @escaping () -> Void = {},
        onCreateRole: @escaping () -> Void = {},
        onValidate: @escaping (RoleEditorDraft) -> Void = { _ in },
        onSaveVersion: @escaping (RoleEditorDraft) -> Void = { _ in },
        onArchiveRole: @escaping (String) -> Void = { _ in },
        onUnarchiveRole: @escaping (String) -> Void = { _ in },
        onActivateVersion: @escaping (String) -> Void = { _ in },
        onExportVersion: @escaping (String) -> Void = { _ in }
    ) {
        self.roles = roles
        self.actions = actions
        self.validationMessages = validationMessages
        self.runtimeError = runtimeError
        self.onClose = onClose
        self.onCreateRole = onCreateRole
        self.onValidate = onValidate
        self.onSaveVersion = onSaveVersion
        self.onArchiveRole = onArchiveRole
        self.onUnarchiveRole = onUnarchiveRole
        self.onActivateVersion = onActivateVersion
        self.onExportVersion = onExportVersion

        let initialRole = roles.first(where: { $0.id == selectedRoleId }) ?? roles.first
        _selectedRoleId = State(initialValue: initialRole?.id ?? "")
        _selectedVersionId = State(initialValue: initialRole?.versions.first(where: { $0.isActive })?.id ?? initialRole?.versions.first?.id ?? "")
        _label = State(initialValue: initialRole?.label ?? "")
        _descriptionText = State(initialValue: initialRole?.descriptionText ?? "")
        _instructionText = State(initialValue: initialRole?.instructionText ?? "")
        _authorityDecisions = State(initialValue: initialRole?.authorityDecisions ?? [:])
    }

    var body: some View {
        VStack(spacing: 0) {
            RoleEditorHeader(
                role: selectedRole,
                canSave: canSave,
                onClose: onClose,
                onValidate: {
                    onValidate(draft)
                },
                onSaveVersion: submit
            )

            Divider().opacity(0.6)

            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 26) {
                        VStack(alignment: .leading, spacing: 24) {
                            RoleListView(
                                roles: roles,
                                selectedRoleId: $selectedRoleId,
                                onCreateRole: onCreateRole,
                                onArchiveRole: onArchiveRole,
                                onUnarchiveRole: onUnarchiveRole
                            )

                            RoleVersionListView(
                                role: selectedRole,
                                selectedVersionId: $selectedVersionId,
                                onActivateVersion: onActivateVersion,
                                onExportVersion: onExportVersion
                            )
                        }
                        .frame(width: 280, alignment: .topLeading)

                        RoleDraftEditor(
                            roleId: selectedRoleId,
                            label: $label,
                            descriptionText: $descriptionText,
                            instructionText: $instructionText,
                            validationMessages: visibleValidationMessages,
                            runtimeError: runtimeError
                        )
                        .frame(minWidth: 360, maxWidth: 560, alignment: .topLeading)

                        ScrollView(.vertical) {
                            RoleAuthorityEditorView(
                                actions: actions,
                                decisions: $authorityDecisions
                            )
                        }
                        .frame(minWidth: 420, maxWidth: 520, maxHeight: 680, alignment: .topLeading)
                    }

                    VStack(alignment: .leading, spacing: 24) {
                        RoleListView(
                            roles: roles,
                            selectedRoleId: $selectedRoleId,
                            onCreateRole: onCreateRole,
                            onArchiveRole: onArchiveRole,
                            onUnarchiveRole: onUnarchiveRole
                        )

                        RoleVersionListView(
                            role: selectedRole,
                            selectedVersionId: $selectedVersionId,
                            onActivateVersion: onActivateVersion,
                            onExportVersion: onExportVersion
                        )

                        RoleDraftEditor(
                            roleId: selectedRoleId,
                            label: $label,
                            descriptionText: $descriptionText,
                            instructionText: $instructionText,
                            validationMessages: visibleValidationMessages,
                            runtimeError: runtimeError
                        )

                        RoleAuthorityEditorView(
                            actions: actions,
                            decisions: $authorityDecisions
                        )
                    }
                    .frame(maxWidth: 680, alignment: .topLeading)
                }
                .padding(24)
                .frame(maxWidth: 1420)
                .frame(maxWidth: .infinity)
            }
            .background(RoleEditorBackground())
        }
        .frame(minWidth: 900, minHeight: 720)
        .onChange(of: selectedRoleId) { _, newRoleId in
            loadRole(newRoleId)
        }
    }

    private func submit() {
        submitted = true
        guard canSave else {
            return
        }
        onSaveVersion(draft)
    }

    private func loadRole(_ roleId: String) {
        guard let role = roles.first(where: { $0.id == roleId }) else {
            return
        }
        selectedVersionId = role.versions.first(where: { $0.isActive })?.id ?? role.versions.first?.id ?? ""
        label = role.label
        descriptionText = role.descriptionText
        instructionText = role.instructionText
        authorityDecisions = role.authorityDecisions
        submitted = false
    }
}

private struct RoleEditorHeader: View {
    let role: RuntimeRole?
    let canSave: Bool
    let onClose: () -> Void
    let onValidate: () -> Void
    let onSaveVersion: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Role Editor")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text(role == nil ? "Create or select a runtime role to edit instructions and authority." : "Edit instructions and action policy for \(role?.label ?? "selected role").")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer()

            Button("Close") {
                onClose()
            }
            .buttonStyle(.bordered)

            Button("Validate") {
                onValidate()
            }
            .buttonStyle(.bordered)
            .disabled(role == nil)
            .help(role == nil ? "Select a role before validating." : "Validate the current role draft.")

            Button("Save Version") {
                onSaveVersion()
            }
            .buttonStyle(.borderedProminent)
            .disabled(!canSave)
            .help(canSave ? "Save an immutable role version." : "Resolve required role fields before saving.")
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.regularMaterial)
    }
}

private struct RoleDraftEditor: View {
    let roleId: String
    @Binding var label: String
    @Binding var descriptionText: String
    @Binding var instructionText: String
    let validationMessages: [String]
    let runtimeError: RoleEditorRuntimeError?

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            RoleEditorSection(title: "Role draft") {
                RoleEditorReadOnlyField(
                    title: "Role id",
                    value: roleId,
                    placeholder: "No role selected",
                    help: "Stable runtime role identifier. New versions keep this role id."
                )

                RoleEditorTextField(
                    title: "Label",
                    text: $label,
                    prompt: "Runtime allow",
                    help: "Human-readable role label shown in project and session defaults."
                )

                RoleEditorTextField(
                    title: "Description",
                    text: $descriptionText,
                    prompt: "General role for project work",
                    help: "Short summary for role selection."
                )
            }

            RoleEditorSection(title: "Instructions") {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Instruction text")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    TextEditor(text: $instructionText)
                        .font(.system(size: 13, design: .monospaced))
                        .scrollContentBackground(.hidden)
                        .padding(10)
                        .frame(minHeight: 240)
                        .background(Color.primary.opacity(0.045))
                        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .overlay {
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                        }

                    Text("Saved into immutable role versions as inline instruction text. No prompt file is created here.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            if !validationMessages.isEmpty {
                RoleEditorNotice(title: "Before saving", messages: validationMessages, tone: .warning)
            }

            if let runtimeError {
                RoleEditorNotice(title: "Runtime error", messages: [runtimeError.message, runtimeError.recovery ?? ""].filter { !$0.isEmpty }, tone: .critical)
            }
        }
    }
}

private struct RoleEditorSection<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(1.0)

            VStack(alignment: .leading, spacing: 16) {
                content
            }
        }
    }
}

private struct RoleEditorTextField: View {
    let title: String
    @Binding var text: String
    let prompt: String
    let help: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            TextField(prompt, text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .background(Color.primary.opacity(0.045))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                }

            Text(help)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct RoleEditorReadOnlyField: View {
    let title: String
    let value: String
    let placeholder: String
    let help: String

    private var displayValue: String {
        value.isEmpty ? placeholder : value
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            Text(displayValue)
                .font(.system(size: 14, design: .monospaced))
                .foregroundStyle(value.isEmpty ? .tertiary : .secondary)
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.035))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.08), lineWidth: 1)
                }

            Text(help)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
        }
    }
}

private struct RoleEditorNotice: View {
    let title: String
    let messages: [String]
    let tone: RoleEditorNoticeTone

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(tone.color)

            ForEach(messages, id: \.self) { message in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "circle.fill")
                        .font(.system(size: 5, weight: .bold))
                        .foregroundStyle(tone.color)

                    Text(message)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(12)
        .background(tone.backgroundColor)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

private struct RoleEditorBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.030, green: 0.040, blue: 0.056)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.026),
                    Color.clear,
                    Color.black.opacity(0.10)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }
}

private enum RoleEditorNoticeTone {
    case warning
    case critical

    var color: Color {
        switch self {
        case .warning:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .critical:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var backgroundColor: Color {
        switch self {
        case .warning:
            return Color(red: 0.130, green: 0.095, blue: 0.050)
        case .critical:
            return Color(red: 0.130, green: 0.055, blue: 0.055)
        }
    }
}

struct RuntimeRole: Identifiable {
    let id: String
    let label: String
    let descriptionText: String
    let activeVersionLabel: String
    let isArchived: Bool
    let versions: [RuntimeRoleVersion]
    var instructionText: String = ""
    var authorityDecisions: [String: RolePolicyDecision] = [:]
}

struct RuntimeRoleVersion: Identifiable {
    let id: String
    let label: String
    let summary: String
    let createdText: String
    let isActive: Bool
}

struct RoleActionAuthority: Identifiable {
    let id: String
    let label: String
    let groupLabel: String
    let descriptionText: String
}

struct RoleEditorDraft: Equatable {
    let roleId: String
    let label: String
    let descriptionText: String
    let instructionText: String
    let authorityDecisions: [String: RolePolicyDecision]
}

enum RolePolicyDecision: String, CaseIterable, Identifiable {
    case off
    case allow
    case deny
    case ownerApproval
    case orchestratorApproval

    var id: String {
        rawValue
    }

    var title: String {
        switch self {
        case .off:
            return "Off / absent"
        case .allow:
            return "Allow"
        case .deny:
            return "Deny"
        case .ownerApproval:
            return "Owner approval"
        case .orchestratorApproval:
            return "Orchestrator approval"
        }
    }
}

struct RoleEditorRuntimeError {
    let message: String
    let recovery: String?
}

#Preview(traits: .landscapeLeft) {
    let actions = [
        RoleActionAuthority(id: "command.registry.preview", label: "Preview command registry change", groupLabel: "command.registry", descriptionText: "Validate and diff proposed command registry changes."),
        RoleActionAuthority(id: "command.registry.decide", label: "Decide command registry request", groupLabel: "command.registry", descriptionText: "Approve or deny a pending registry request."),
        RoleActionAuthority(id: "command.registry.apply", label: "Apply command registry change", groupLabel: "command.registry", descriptionText: "Write an approved command definition into the registry."),
        RoleActionAuthority(id: "workflow.memory.read", label: "Read workflow memory", groupLabel: "workflow.memory", descriptionText: "Inspect persisted workflow memory for the current project."),
        RoleActionAuthority(id: "workflow.memory.update", label: "Update workflow memory", groupLabel: "workflow.memory", descriptionText: "Write project workflow memory after review."),
        RoleActionAuthority(id: "process.terminate", label: "Terminate process", groupLabel: "process", descriptionText: "Stop a running async process or service."),
        RoleActionAuthority(id: "session.fork", label: "Fork session", groupLabel: "session", descriptionText: "Create a new session from the selected session context."),
        RoleActionAuthority(id: "session.archive", label: "Archive session", groupLabel: "session", descriptionText: "Remove a session from the active workspace list.")
    ]

    let roles = [
        RuntimeRole(
            id: "runtime-allow",
            label: "Runtime allow",
            descriptionText: "General runtime role for normal project work.",
            activeVersionLabel: "v4 active",
            isArchived: false,
            versions: [
                RuntimeRoleVersion(id: "runtime-allow-v4", label: "v4", summary: "Command registry changes require owner approval.", createdText: "Today, 9:40 AM", isActive: true),
                RuntimeRoleVersion(id: "runtime-allow-v3", label: "v3", summary: "Added workflow memory authority.", createdText: "Yesterday, 4:12 PM", isActive: false)
            ],
            instructionText: """
            You are operating inside the Agent Runtime. Keep changes scoped to the selected project, explain risky operations before requesting approval, and prefer small reversible edits.
            """,
            authorityDecisions: [
                "command.registry.preview": .allow,
                "command.registry.decide": .ownerApproval,
                "command.registry.apply": .ownerApproval,
                "workflow.memory.read": .allow,
                "workflow.memory.update": .orchestratorApproval,
                "process.terminate": .ownerApproval,
                "session.fork": .allow,
                "session.archive": .deny
            ]
        ),
        RuntimeRole(
            id: "requirements-reviewer",
            label: "Requirements reviewer",
            descriptionText: "Reviews requirement claims and image evidence.",
            activeVersionLabel: "v2 active",
            isArchived: false,
            versions: [
                RuntimeRoleVersion(id: "requirements-reviewer-v2", label: "v2", summary: "Restricted process authority.", createdText: "Jun 26, 2026", isActive: true)
            ],
            instructionText: "Review claims against owner-visible evidence. Do not approve unverifiable UI changes.",
            authorityDecisions: [
                "workflow.memory.read": .allow,
                "process.terminate": .deny,
                "session.archive": .deny
            ]
        ),
        RuntimeRole(
            id: "legacy-debugger",
            label: "Legacy debugger",
            descriptionText: "Old debugger role retained for audit history.",
            activeVersionLabel: "v1 active",
            isArchived: true,
            versions: [
                RuntimeRoleVersion(id: "legacy-debugger-v1", label: "v1", summary: "Initial imported role.", createdText: "Jun 20, 2026", isActive: true)
            ],
            instructionText: "Legacy role retained for reference.",
            authorityDecisions: [:]
        )
    ]

    RoleEditorView(
        roles: roles,
        actions: actions,
        selectedRoleId: "runtime-allow",
        validationMessages: []
    )
    .frame(width: 1280, height: 780)
    .preferredColorScheme(.dark)
}
