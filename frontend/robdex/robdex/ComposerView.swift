//
//  ComposerView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct ComposerView: View {
    @State private var message = ""
    let contextRemaining: Double

    var body: some View {
        VStack(spacing: 0) {
            ContextRemainingBar(remaining: contextRemaining)
                .frame(height: 6)
            
            VStack(alignment: .leading, spacing: 12) {
                TextField("Message selected session…", text: $message, axis: .vertical)
                    .textFieldStyle(.plain)
                    .font(.system(size: 15))
                    .lineSpacing(3)
                    .frame(alignment: .topLeading)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .background(Color.primary.opacity(0.045))
                    .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                    }

                HStack(alignment: .center, spacing: 8) {
                    ComposerIconButton(title: "Attach image", systemImage: "photo")
                    ComposerIconButton(title: "Session settings", systemImage: "slider.horizontal.3")

                    Spacer()

                    Button {
                    } label: {
                        Label("Send", systemImage: "arrow.up")
                            .labelStyle(.titleAndIcon)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.regular)
                    .disabled(message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .padding(.horizontal, 18)
            .padding(.top, 16)
            .padding(.bottom, 14)
        }
        .background(.regularMaterial)
        .overlay(alignment: .top) {
            Divider().opacity(0.6)
        }
    }
}

private struct ComposerIconButton: View {
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

private struct ContextRemainingBar: View {
    let remaining: Double

    private var clampedRemaining: Double {
        min(max(remaining, 0), 1)
    }

    private var barColor: Color {
        let red = Color(red: 1.00, green: 0.34, blue: 0.28)
        let amber = Color(red: 0.98, green: 0.67, blue: 0.25)
        let blue = Color(red: 0.24, green: 0.56, blue: 0.96)

        if clampedRemaining > 0.45 {
            return blue
        }

        if clampedRemaining > 0.20 {
            return amber
        }

        return red
    }

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Color.primary.opacity(0.085)

                Rectangle()
                    .fill(barColor)
                    .frame(width: proxy.size.width * clampedRemaining)
                    .shadow(color: barColor.opacity(0.32), radius: 8, x: 0, y: -1)
            }
            .accessibilityLabel("Context remaining")
            .accessibilityValue("\(Int(clampedRemaining * 100)) percent")
        }
    }
}

#Preview(traits: .landscapeLeft) {
    VStack(spacing: 0) {
        Spacer()
        ComposerView(contextRemaining: 0.64)
    }
    .frame(width: 820, height: 260)
    .background(Color(red: 0.030, green: 0.040, blue: 0.056))
    .preferredColorScheme(.dark)
}
