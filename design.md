# Design

Theme: Limited space


## Spaceship confined in a single solar system. Hard science.

### Introduction

You and the crew wake up from glassification sleep.
After a quick assessment, you find out that the navigation planner made a mistake.
The ship is almost out of fuel, and you seem to be in the middle of nowhere.
In the vicinity there is only a small star system.
You recall the details from the library:
You have to find fuel and food to restart the journey, be quick!

### Gameplay

Star system map on a graph, can only visit some planets as others are not interesting.
Collect fuel for fusion drive. It needs to be some strange chemical.
Scout various planets to search for Helium-4 (atmosphere of Saturn and Uranus).
Each jump is a long endeavor, consumes food and supplies. 
You win by having enough resources to restart the voyage.
The ship has short range rocket shuttles to go to planets.

### Map

Hand drawn small graph. Needs to be claustrophobic.

### Economy

Each node has a couple slots.
In each slot you can construct a building.
Going to another node passes one or more turns, consumes fuel.
You can pass the turn.
In each node there can be an unlimited amount of materials.
Acting on a planet requires 1 rocket.
Each turn your crew eats 4 food.

Starting resources:
10 fusion,
20 food,
10 materials,
20 rocket,

Ship buildings:
solar array
hydroponic farm

Buildings:

- solar array:
+3 power/turn on surface, +5 power/turn in space (2M)

- hydroponic farm:
+2 food/turn, -2 power/turn, (3M)

- atmosphere harvester: (planet only) 
+8 fusion/3 turn, -4 power/turn (20M)

- chemical plant: (planet only) 
+4 rocket/turn, -1 materials/turn, (3M)

- bacteria farm:
+3 food/turn, (1M) decays in 3 turns

- planet farm: (planet only)
+20 food/5 turns, (15M)

- asteroid mine: (asteroid only)
+5 material/2 turn, -2 rocket/turn (4M)

- quarry:
+60 materials/3 turns, -5 power/turn (18M), decays in 6 turns

- fusion generator:
+10 power/turn, -1 fusion/turn (10M)

- rocket generator:
+4 power/turn, -1 rocket/turn (1M)

- burner generator: 
-1 food/turn, +2 power/turn (2M), decays in 3 turns

```
enum Resource {
    Fusion,
    Rocket,
    Food,
    Material,
    Power,
}
```
