// Screen Y points down. Face order and rotation masks are supplied by Rust.
export function createHexLayout(contract, radius = 34) {
  const position = ([q, r]) => [
    Math.sqrt(3) * radius * (q + r * 0.5),
    1.5 * radius * r,
  ];
  const direction = (index) => {
    const [x, y] = position(contract.faces[index].delta);
    const length = Math.hypot(x, y);
    return [x / length, y / length];
  };
  const angle = (index) => {
    const [x, y] = direction(index);
    return (Math.atan2(y, x) * 180) / Math.PI;
  };
  const shape = (name) => contract.shapes.find((entry) => entry.name === name);
  const mask = (name, rotation) =>
    shape(name).rotations[((rotation % 6) + 6) % 6];
  const appearance = (doors) => {
    for (const name of ["corridor", "bend", "junction"]) {
      const rotation = shape(name).rotations.indexOf(doors);
      if (rotation !== -1) return { name, rotation, exact: true };
    }
    // Other WFC signatures retain exact procedural walls; painted stone is dressing.
    return { name: "junction", rotation: 0, exact: false };
  };
  return { position, direction, angle, mask, appearance, contract };
}
