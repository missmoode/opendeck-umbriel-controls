// Layouts enabled for the cycle, in the order sent to Umbriel.
(() => {
  const { useSettings } = SDPIComponents;
  const modes = ["scrolling", "dwindle", "master", "stacking"];
  const names = {
    scrolling: "Scrolling",
    dwindle: "Dwindle",
    master: "Master",
    stacking: "Stacking",
  };
  const container = document.getElementById("layout-cycle");
  let cycle = [...modes];

  function load(value) {
    if (!Array.isArray(value) || value.length === 0) return [...modes];
    const items = value.filter((mode) => modes.includes(mode));
    return items.length >= 2 && new Set(items).size === items.length
      ? items
      : [...modes];
  }

  const [getCycle, setCycle] = useSettings(
    "cycle",
    (value) => {
      cycle = load(value);
      render();
    },
    0,
  );

  function save() {
    setCycle([...cycle]);
    render();
  }

  function render() {
    container.replaceChildren();
    const ordered = [
      ...cycle,
      ...modes.filter((mode) => !cycle.includes(mode)),
    ];
    for (const mode of ordered) {
      const active = cycle.includes(mode);
      const index = cycle.indexOf(mode);
      const row = document.createElement("div");
      row.className = "uc-layout-row";

      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.checked = active;
      toggle.disabled = active && cycle.length <= 2;
      toggle.addEventListener("change", () => {
        if (toggle.checked) cycle.push(mode);
        else cycle = cycle.filter((item) => item !== mode);
        save();
      });
      const label = document.createElement("label");
      label.append(toggle, document.createTextNode(names[mode]));
      row.append(label);

      for (const [symbol, offset] of [
        ["↑", -1],
        ["↓", 1],
      ]) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = symbol;
        button.title = `Move ${names[mode]} ${offset < 0 ? "up" : "down"}`;
        button.disabled =
          !active || index + offset < 0 || index + offset >= cycle.length;
        button.addEventListener("click", () => {
          [cycle[index], cycle[index + offset]] = [
            cycle[index + offset],
            cycle[index],
          ];
          save();
        });
        row.append(button);
      }
      container.append(row);
    }
  }

  getCycle().then((value) => {
    cycle = load(value);
    render();
  });
  render();
})();
