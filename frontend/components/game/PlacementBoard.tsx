"use client";

import React, { useState } from 'react';
import { DndContext, DragEndEvent, DragOverlay, MouseSensor, TouchSensor, useSensor, useSensors, DragStartEvent } from '@dnd-kit/core';
import { snapCenterToCursor } from '@dnd-kit/modifiers';
import { GridCell } from './GridCell';
import { Ship } from './Ship';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { RotateCcw, Check, Shuffle } from 'lucide-react';
import { cn } from '@/lib/utils';

// Ship Types
type Orientation = 'horizontal' | 'vertical';
interface ShipData {
    id: string;
    name: string;
    size: number;
    orientation: Orientation;
    placed: boolean;
    x: number;
    y: number;
}

const INITIAL_SHIPS: ShipData[] = [
    { id: 'carrier', name: 'Carrier', size: 5, orientation: 'horizontal', placed: false, x: -1, y: -1 },
    { id: 'battleship', name: 'Battleship', size: 4, orientation: 'horizontal', placed: false, x: -1, y: -1 },
    { id: 'cruiser', name: 'Cruiser', size: 3, orientation: 'horizontal', placed: false, x: -1, y: -1 },
    { id: 'submarine', name: 'Submarine', size: 3, orientation: 'horizontal', placed: false, x: -1, y: -1 },
    { id: 'destroyer', name: 'Destroyer', size: 2, orientation: 'horizontal', placed: false, x: -1, y: -1 },
];

export function PlacementBoard({ onConfirm }: { onConfirm: (ships: ShipData[]) => void }) {
    const [ships, setShips] = useState<ShipData[]>(INITIAL_SHIPS);
    const [activeId, setActiveId] = useState<string | null>(null);

    const sensors = useSensors(
        useSensor(MouseSensor, { activationConstraint: { distance: 5 } }),
        useSensor(TouchSensor, { activationConstraint: { delay: 100, tolerance: 5 } })
    );

    const handleDragStart = (event: DragStartEvent) => {
        const shipId = event.active.id as string;
        setActiveId(shipId);
        // If ship was placed, unplace it so it can be re-dragged
        setShips(prev => prev.map(s =>
            s.id === shipId ? { ...s, placed: false, x: -1, y: -1 } : s
        ));
    };

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;
        setActiveId(null);

        if (over) {
            const [x, y] = over.id.toString().split('-').map(Number);
            const ship = ships.find(s => s.id === active.id);

            if (ship && isValidPlacement(ship, x, y, ships.filter(s => s.id !== ship.id))) {
                setShips(prev => prev.map(s =>
                    s.id === ship.id
                        ? { ...s, placed: true, x, y }
                        : s
                ));
            }
        }
    };

    const rotateShip = (id: string) => {
        setShips(prev => {
            const ship = prev.find(s => s.id === id);
            if (!ship) return prev;

            const newOrientation = ship.orientation === 'horizontal' ? 'vertical' : 'horizontal';
            // If placed, check if rotation is valid
            if (ship.placed) {
                if (isValidPlacement({ ...ship, orientation: newOrientation }, ship.x, ship.y, prev.filter(s => s.id !== id))) {
                    return prev.map(s => s.id === id ? { ...s, orientation: newOrientation } : s);
                }
                return prev; // Invalid rotation
            }

            return prev.map(s => s.id === id ? { ...s, orientation: newOrientation } : s);
        });
    };

    const isValidPlacement = (ship: ShipData, x: number, y: number, otherShips: ShipData[]) => {
        // Check bounds
        if (ship.orientation === 'horizontal') {
            if (x + ship.size > 10) return false;
        } else {
            if (y + ship.size > 10) return false;
        }

        // Check collisions
        const shipCells = new Set();
        for (let i = 0; i < ship.size; i++) {
            const cx = ship.orientation === 'horizontal' ? x + i : x;
            const cy = ship.orientation === 'vertical' ? y + i : y;
            shipCells.add(`${cx}-${cy}`);
        }

        for (const other of otherShips) {
            if (!other.placed) continue;
            for (let i = 0; i < other.size; i++) {
                const ox = other.orientation === 'horizontal' ? other.x + i : other.x;
                const oy = other.orientation === 'vertical' ? other.y + i : other.y;
                if (shipCells.has(`${ox}-${oy}`)) return false;
            }
        }

        return true;
    };

    // Randomize fleet placement
    const randomizeFleet = () => {
        let newShips = INITIAL_SHIPS.map(s => ({ ...s }));
        const placedShips: ShipData[] = [];

        for (const ship of newShips) {
            let placed = false;
            let attempts = 0;

            while (!placed && attempts < 100) {
                attempts++;
                const orientation: Orientation = Math.random() > 0.5 ? 'horizontal' : 'vertical';
                const maxX = orientation === 'horizontal' ? 10 - ship.size : 9;
                const maxY = orientation === 'vertical' ? 10 - ship.size : 9;
                const x = Math.floor(Math.random() * (maxX + 1));
                const y = Math.floor(Math.random() * (maxY + 1));

                const testShip = { ...ship, orientation, x, y };
                if (isValidPlacement(testShip, x, y, placedShips)) {
                    ship.orientation = orientation;
                    ship.x = x;
                    ship.y = y;
                    ship.placed = true;
                    placedShips.push(ship);
                    placed = true;
                }
            }
        }

        setShips(newShips);
    };

    const activeShip = activeId ? ships.find(s => s.id === activeId) : null;

    return (
        <div className="flex flex-col lg:flex-row gap-8 items-start justify-center p-6 w-full max-w-5xl mx-auto">
            <DndContext
                sensors={sensors}
                onDragStart={handleDragStart}
                onDragEnd={handleDragEnd}
                modifiers={[snapCenterToCursor]}
            >
                {/* Fleet Dock */}
                <Card className="w-full lg:w-64 p-6 bg-black/40 border-white/10 backdrop-blur-md">
                    <div className="flex justify-between items-center mb-4">
                        <h3 className="font-heading text-lg text-primary">FLEET DOCK</h3>
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={randomizeFleet}
                            className="text-xs h-7 px-2 border-primary/30 hover:bg-primary/20"
                        >
                            <Shuffle className="w-3 h-3 mr-1" /> Random
                        </Button>
                    </div>
                    <div className="flex flex-col gap-6 min-h-[300px]">
                        {ships.filter(s => !s.placed).map(ship => (
                            <div key={ship.id} className="relative group">
                                <Ship id={ship.id} size={ship.size} orientation={ship.orientation} onRotate={() => rotateShip(ship.id)} />
                            </div>
                        ))}
                        {ships.every(s => s.placed) && (
                            <div className="text-zinc-500 italic text-center py-10">All Units Deployed</div>
                        )}
                    </div>
                </Card>

                {/* The Grid */}
                <div className="flex flex-col gap-4">
                    <div className="bg-black/50 p-4 rounded-xl border border-white/10 shadow-2xl relative">
                        <div className="grid grid-cols-10 gap-0 border border-white/5">
                            {Array.from({ length: 100 }).map((_, i) => {
                                const x = i % 10;
                                const y = Math.floor(i / 10);

                                // Check if a ship is here
                                const shipHere = ships.find(s =>
                                    s.placed &&
                                    (s.orientation === 'horizontal'
                                        ? y === s.y && x >= s.x && x < s.x + s.size
                                        : x === s.x && y >= s.y && y < s.y + s.size)
                                );

                                return (
                                    <div key={`${x}-${y}`} className="relative w-10 h-10 border-white/5 border-[0.5px]">
                                        <GridCell id={`${x}-${y}`} x={x} y={y} />
                                        {shipHere && shipHere.x === x && shipHere.y === y && (
                                            <div className="absolute top-0 left-0 z-10">
                                                <Ship
                                                    id={shipHere.id}
                                                    size={shipHere.size}
                                                    orientation={shipHere.orientation}
                                                    placed={true}
                                                    onRotate={() => rotateShip(shipHere.id)}
                                                />
                                            </div>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    </div>

                    <div className="flex justify-between items-center">
                        <Button variant="ghost" onClick={() => setShips(INITIAL_SHIPS)}>
                            <RotateCcw className="w-4 h-4 mr-2" /> Reset
                        </Button>
                        <div className="text-zinc-500 font-mono text-sm">
                            {ships.filter(s => s.placed).length} / 5 PLACED
                        </div>
                        <div onClick={() => !ships.every(s => s.placed) && alert("Deploy all ships first!")}>
                            <Button
                                variant="neon"
                                disabled={!ships.every(s => s.placed)}
                                onClick={() => onConfirm(ships)}
                                className="w-40"
                            >
                                <Check className="w-4 h-4 mr-2" /> CONFIRM
                            </Button>
                        </div>
                    </div>
                </div>

                <DragOverlay>
                    {activeShip ? (
                        <Ship id={activeShip.id} size={activeShip.size} orientation={activeShip.orientation} />
                    ) : null}
                </DragOverlay>

            </DndContext>
        </div>
    );
}
