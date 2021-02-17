use fenris::assembly::global::{CsrAssembler, CsrParAssembler};
use fenris::assembly2::{
    ElementEllipticAssembler, ElementSourceAssembler, EllipticContraction, EllipticOperator,
    Operator, SerialVectorAssembler, SourceFunction, UniformQuadratureTable,
};
use fenris::mesh::QuadMesh2d;
use fenris::nalgebra::{DVector, DVectorSlice, MatrixMN, Point2, VectorN, U1, U2};
use fenris::procedural::create_unit_square_uniform_quad_mesh_2d;
use fenris::quadrature::quad_quadrature_strength_5_f64;
use fenris_sparse::CsrMatrix;
use nalgebra::{Point, Vector1};
use std::error::Error;
use std::sync::Arc;

pub struct PoissonOperator2d;

impl Operator<f64> for PoissonOperator2d {
    type SolutionDim = U1;
    type Data = ();
}

impl EllipticOperator<f64, U2> for PoissonOperator2d {
    fn compute_elliptic_term(
        &self,
        gradient: &MatrixMN<f64, U2, Self::SolutionDim>,
        _data: &Self::Data,
    ) -> MatrixMN<f64, U2, Self::SolutionDim> {
        *gradient
    }
}

impl EllipticContraction<f64, U2> for PoissonOperator2d {
    fn contract(
        &self,
        _gradient: &MatrixMN<f64, U2, Self::SolutionDim>,
        _data: &Self::Data,
        a: &VectorN<f64, U2>,
        b: &VectorN<f64, U2>,
    ) -> MatrixMN<f64, Self::SolutionDim, Self::SolutionDim> {
        Vector1::new(a.dot(&b))
    }
}

struct Source;

impl Operator<f64> for Source {
    type SolutionDim = U1;
    type Data = ();
}

impl SourceFunction<f64, U2> for Source {
    fn evaluate(&self, _coords: &Point2<f64>, _data: &Self::Data) -> Vector1<f64> {
        // TODO: Use a more interesting function than a constant function
        Vector1::new(1.0)
    }
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // TODO: Make it easy to construct triangle meshes as well.
    // Need to make it easy to convert between different meshes, such as Quad2d -> Tri2d
    let mesh: QuadMesh2d<f64> = create_unit_square_uniform_quad_mesh_2d(3);
    let op = PoissonOperator2d;

    let (weights, points) = quad_quadrature_strength_5_f64();
    // TODO: Use Point for quadratures
    let points = points.into_iter().map(Point::from).collect();
    let quadrature = UniformQuadratureTable::from_points_and_weights(points, weights);

    let u = DVector::<f64>::zeros(mesh.vertices().len());

    // TODO: Build API or something for the elliptic assembler?
    let element_assembler = ElementEllipticAssembler {
        space: &mesh,
        op: &op,
        qtable: &quadrature,
        u: DVectorSlice::from(&u),
    };

    // TODO: CsrAssembler is not able to assemble patterns atm. So we use par assembler for the
    // pattern
    let matrix_assembler = CsrParAssembler::<f64>::default();
    let vector_assembler = SerialVectorAssembler::<f64>::default();
    let pattern = matrix_assembler.assemble_pattern(&element_assembler);
    let mut matrix_assembler = CsrAssembler::default();
    let nnz = pattern.nnz();
    let mut a = CsrMatrix::from_pattern_and_values(Arc::new(pattern), vec![0.0; nnz]);
    // TODO: Need a method that does the whole business of assembling a matrix without
    // needing storage first
    // TODO: Doesn't need to be mutable, does it?
    matrix_assembler.assemble_into_csr(&mut a, &element_assembler)?;

    println!("{}", a.build_dense());

    let source = Source;
    let source_assembler: ElementSourceAssembler<f64, _, _, _> = ElementSourceAssembler {
        space: &mesh,
        qtable: &quadrature,
        source: &source,
        marker: Default::default(),
    };

    let b = vector_assembler.assemble_vector(&source_assembler)?;
    dbg!(b);

    // TODO: Boundary conditions
    // TODO: Solve system

    Ok(())
}
