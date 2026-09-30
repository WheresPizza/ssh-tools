import { useState } from "react";
import { generateSshKey } from "../../lib/tauri";
import type { KeyAlgorithm, KeyGenParams } from "../../lib/tauri";
import { useStore } from "../../stores";

interface KeyGeneratorDialogProps {
  onClose: () => void;
  onCancel: () => void;
}

const ALGORITHMS: { value: KeyAlgorithm; label: string }[] = [
  { value: "Ed25519", label: "Ed25519 (recommended)" },
  { value: "Rsa4096", label: "RSA 4096" },
  { value: "Rsa2048", label: "RSA 2048" },
  { value: "EcdsaP256", label: "ECDSA P-256" },
  { value: "EcdsaP384", label: "ECDSA P-384" },
];

export function KeyGeneratorDialog({ onClose, onCancel }: KeyGeneratorDialogProps) {
  const showToast = useStore((s) => s.showToast);
  const [algorithm, setAlgorithm] = useState<KeyAlgorithm>("Ed25519");
  const [comment, setComment] = useState("");
  const [filename, setFilename] = useState("id_ed25519");
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [generating, setGenerating] = useState(false);

  const handleAlgorithmChange = (alg: KeyAlgorithm) => {
    setAlgorithm(alg);
    const defaults: Record<KeyAlgorithm, string> = {
      Ed25519: "id_ed25519",
      Rsa4096: "id_rsa",
      Rsa2048: "id_rsa",
      EcdsaP256: "id_ecdsa",
      EcdsaP384: "id_ecdsa",
    };
    setFilename(defaults[alg] ?? "id_key");
  };

  const handleGenerate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (generating) return;
    if (passphrase !== passphraseConfirm) {
      showToast("Passphrases do not match", "error");
      return;
    }

    setGenerating(true);
    try {
      const params: KeyGenParams = {
        algorithm,
        comment,
        filename,
        passphrase: passphrase || null,
      };
      await generateSshKey(params);
      showToast(`Key '${filename}' generated successfully`, "success");
      onClose();
    } catch (e) {
      showToast(`Failed to generate key: ${e}`, "error");
    } finally {
      setGenerating(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-card border border-border rounded-lg shadow-xl p-6 max-w-md w-full mx-4 max-h-[90vh] overflow-y-auto">
        <h3 className="text-base font-semibold mb-4">Generate SSH Key</h3>

        <form onSubmit={handleGenerate} className="space-y-4">
          <div>
            <label htmlFor="key-algorithm" className="block text-sm font-medium mb-1.5">Algorithm</label>
            <select
              id="key-algorithm"
              value={algorithm}
              onChange={(e) => handleAlgorithmChange(e.target.value as KeyAlgorithm)}
              className="input-field"
            >
              {ALGORITHMS.map((a) => (
                <option key={a.value} value={a.value}>{a.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="key-filename" className="block text-sm font-medium mb-1.5">Filename</label>
            <input
              required
              id="key-filename"
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
              placeholder="id_ed25519"
              className="input-field font-mono"
            />
            <p className="text-xs text-muted-foreground mt-1">Saved as {filename} in the active SSH workspace</p>
          </div>
          <div>
            <label htmlFor="key-comment" className="block text-sm font-medium mb-1.5">Comment (optional)</label>
            <input
              id="key-comment"
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              placeholder="user@host"
              className="input-field"
            />
          </div>
          <div>
            <label htmlFor="key-passphrase" className="block text-sm font-medium mb-1.5">Passphrase (optional)</label>
            <input
              type="password"
              id="key-passphrase"
              value={passphrase}
              onChange={(e) => {
                setPassphrase(e.target.value);
                if (!e.target.value) setPassphraseConfirm("");
              }}
              placeholder="Leave empty for no passphrase"
              className="input-field"
            />
          </div>
          {passphrase && (
            <div>
              <label htmlFor="key-passphraseConfirm" className="block text-sm font-medium mb-1.5">Confirm Passphrase</label>
              <input
                type="password"
                id="key-passphraseConfirm"
              value={passphraseConfirm}
                onChange={(e) => setPassphraseConfirm(e.target.value)}
                placeholder="Confirm passphrase"
                className="input-field"
              />
            </div>
          )}

          <div className="flex gap-2 pt-2">
            <button
              type="submit"
              disabled={generating}
              className="btn btn-primary flex-1 disabled:opacity-50"
            >
              {generating ? "Generating..." : "Generate"}
            </button>
            <button
              type="button"
              disabled={generating}
              onClick={onCancel}
              className="btn"
            >
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
