import { invoke } from "@tauri-apps/api/core";

type Note = { id: number; text: string; at: number };

const form = document.querySelector<HTMLFormElement>("#note-form")!;
const textInput = document.querySelector<HTMLTextAreaElement>("#note-text")!;
const clipButton = document.querySelector<HTMLButtonElement>("#note-clip")!;
const errorEl = document.querySelector<HTMLElement>("#note-error")!;
const listEl = document.querySelector<HTMLUListElement>("#note-list")!;
const emptyEl = document.querySelector<HTMLElement>("#note-empty")!;

const pad = (n: number) => String(n).padStart(2, "0");

function render(notes: Note[]) {
  listEl.replaceChildren(
    ...notes.map((n) => {
      const li = document.createElement("li");
      const text = document.createElement("span");
      text.className = "text note-text";
      text.textContent = n.text;
      const d = new Date(n.at);
      const when = document.createElement("span");
      when.className = "when";
      when.textContent = `${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;

      const copy = document.createElement("button");
      copy.type = "button";
      copy.className = "edit";
      copy.textContent = "コピー";
      copy.addEventListener("click", async () => {
        const ok = await invoke<boolean>("copy_note_to_clipboard", { id: n.id });
        copy.textContent = ok ? "コピーした" : "失敗";
        window.setTimeout(() => (copy.textContent = "コピー"), 1500);
      });
      const del = document.createElement("button");
      del.type = "button";
      del.textContent = "削除";
      del.addEventListener("click", async () => render(await invoke<Note[]>("delete_note", { id: n.id })));
      const buttons = document.createElement("span");
      buttons.className = "row-buttons";
      buttons.append(copy, del);

      li.append(text, when, buttons);
      return li;
    }),
  );
  emptyEl.hidden = notes.length > 0;
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  try {
    render(await invoke<Note[]>("add_note", { text: textInput.value }));
    textInput.value = "";
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

clipButton.addEventListener("click", async () => {
  errorEl.textContent = "";
  try {
    render(await invoke<Note[]>("save_clipboard_note"));
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

render(await invoke<Note[]>("list_notes"));
